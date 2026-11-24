//! Mesh- and compute-shader survey: per container, the program's group shape,
//! LDS, barrier count, and whether any barrier sits in a block only some lanes
//! reach (a lane-dependent branch dominates the path to it). On a host whose
//! wave is narrower than the console's, such a barrier deadlocks the group.
//!
//! `victory meshdump [paths] [--hash <16 hex>] [--isa] [--compute]`

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;
use victory_shader::structure::{Atom, LaneExit};
use victory_shader::{Container, FourCC, HwStage, gfx10, ir, native::StageIo, native_programs, structure};

use crate::shader::{MAX_FILE, sources};

pub fn run(paths: Vec<PathBuf>, hashes: Vec<String>, isa: bool, compute: bool) -> Result<()> {
    let paths = crate::default_paths(paths)?;
    let want: HashSet<String> = hashes.into_iter().collect();
    let mut seen = HashSet::new();
    let mut total = 0usize;
    let mut with_barriers = 0usize;
    let mut divergent = 0usize;
    for root in &paths {
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() || entry.metadata().map_or(0, |m| m.len()) > MAX_FILE {
                continue;
            }
            let Ok(data) = std::fs::read(entry.path()) else {
                continue;
            };
            for source in sources(&data) {
                let Some(bytes) = source.bytes() else { continue };
                let hash = blake3::hash(&bytes).to_hex()[..16].to_owned();
                if !seen.insert(hash.clone()) {
                    continue;
                }
                if !want.is_empty() && !want.contains(&hash) {
                    continue;
                }
                let Ok(c) = Container::parse(&bytes) else { continue };
                let Some(dxil) = c.part(FourCC::DXIL) else { continue };
                let Ok(programs) = native_programs(dxil.data) else { continue };
                for n in programs.into_iter().filter(|p| {
                    p.stage == HwStage::Ms || (compute && p.stage == HwStage::Cs)
                }) {
                    // The group shape line differs by stage; the barrier
                    // analysis below is the same for both.
                    let shape = match &n.io {
                        StageIo::Ms(ms) => format!(
                            "ms threads={} lds={} verts={} prims={}",
                            ms.threads, ms.lds_bytes, ms.max_vertices, ms.max_primitives
                        ),
                        StageIo::Cs(cs) => format!(
                            "cs threads={}x{}x{} lds={}",
                            cs.threads[0], cs.threads[1], cs.threads[2], cs.lds_bytes()
                        ),
                        _ => continue,
                    };
                    total += 1;
                    let Ok(insts) = gfx10::decode(&n.code) else { continue };
                    let barriers: Vec<usize> = insts
                        .iter()
                        .filter(|i| i.name() == Some("s_barrier"))
                        .map(|i| i.offset)
                        .collect();
                    if barriers.is_empty() {
                        println!("{hash} {shape} wave64={} barriers=0", n.wave64);
                        continue;
                    }
                    with_barriers += 1;
                    // Which blocks hold a barrier, and which lane-dependent
                    // branches exist; then whether a barrier block is only
                    // reachable through one side of such a branch.
                    let mut verdict = "uniform-path?";
                    let mut lane_branch_blocks: Vec<(usize, usize, usize)> = Vec::new();
                    if let Ok(p) = ir::build(&n.patched_code(), n.wave64, n.vgprs)
                        && let Ok(st) = structure::structure(&p)
                    {
                        for (b, exit) in st.exits.iter().enumerate() {
                            if let LaneExit::Branch { when, taken, not_taken } = *exit {
                                let uniform = st
                                    .bools
                                    .support(when)
                                    .iter()
                                    .all(|a| matches!(a, Atom::Uniform(_)));
                                if !uniform {
                                    lane_branch_blocks.push((b, taken, not_taken));
                                }
                            }
                        }
                        // Blocks holding s_barrier: match by machine offset.
                        let bar_blocks: Vec<usize> = p
                            .blocks
                            .iter()
                            .enumerate()
                            .filter(|(_, b)| {
                                b.insts.iter().any(|i| i.name() == "s_barrier")
                            })
                            .map(|(k, _)| k)
                            .collect();
                        // Reachability from entry avoiding the "other" side
                        // of each lane branch: if a barrier block is reachable
                        // only when a lane branch takes one side, it diverges.
                        let mut any_divergent = false;
                        for &(b, taken, not_taken) in &lane_branch_blocks {
                            for &(skip, keep) in &[(taken, not_taken), (not_taken, taken)] {
                                let reach = reach_avoiding(&p, b, skip);
                                if bar_blocks.iter().any(|&bb| !reach[bb] && reachable(&p, keep, bb)) {
                                    any_divergent = true;
                                }
                            }
                        }
                        verdict = if any_divergent { "DIVERGENT" } else { "all-lanes" };
                        if any_divergent {
                            divergent += 1;
                        }
                    }
                    println!(
                        "{hash} {shape} wave64={} barriers={} {} lane_branches={}",
                        n.wave64,
                        barriers.len(),
                        verdict,
                        lane_branch_blocks.len()
                    );
                    if isa {
                        for i in &insts {
                            let text = match i.name() {
                                Some(_) => i.text(n.wave64),
                                None => format!("<{:?} {:#x}>", i.encoding, i.opcode),
                            };
                            println!("    {:06x}: {text}", i.offset);
                        }
                    }
                }
            }
        }
    }
    println!("programs {total}, with barriers {with_barriers}, divergent-barrier {divergent}");
    Ok(())
}

/// Blocks reachable from the entry without passing through `skip` (the
/// branch block `from` itself is allowed; its edge to `skip` is not).
fn reach_avoiding(p: &ir::Program, from: usize, skip: usize) -> Vec<bool> {
    let mut seen = vec![false; p.blocks.len()];
    let mut stack = vec![0usize];
    while let Some(b) = stack.pop() {
        if seen[b] {
            continue;
        }
        seen[b] = true;
        for s in succs(p, b) {
            if b == from && s == skip {
                continue;
            }
            if !seen[s] {
                stack.push(s);
            }
        }
    }
    seen
}

fn reachable(p: &ir::Program, from: usize, to: usize) -> bool {
    let mut seen = vec![false; p.blocks.len()];
    let mut stack = vec![from];
    while let Some(b) = stack.pop() {
        if b == to {
            return true;
        }
        if seen[b] {
            continue;
        }
        seen[b] = true;
        stack.extend(succs(p, b));
    }
    false
}

fn succs(p: &ir::Program, b: usize) -> Vec<usize> {
    p.blocks[b].exit.successors()
}
