//! Regression guard for the Series S upscaler's ACCC container (AMD's ACO
//! compiler), shader `8e614708143ef917` of the own13_litS corpus.
//!
//! The container parse, the FSPX program extraction and the RDNA instruction
//! decode must all complete without panics on this real-world blob. Full
//! translation is *not* required to succeed yet (the ACO scalar loop over a
//! lane mask still returns `Unsupported`); a graceful `Err` is fine, a panic
//! or a malformed-structure error is not.
//!
//! The fixture is a copy of a title shader kept with the test so the suite
//! stays offline and path-independent (the old dump_accc.rs read the corpus
//! by absolute path and was removed for that reason).

use victory_shader::{Container, FourCC, ShaderError, container_programs};

const BLOB: &[u8] = include_bytes!("data/8e614708143ef917.compute.cdxil");

#[test]
fn dump_accc_regression() {
    // 1. The DXBC wrapper parses and carries an ACCC part, not DXIL.
    let c = Container::parse(BLOB).expect("the container parses");
    assert!(c.part(FourCC::DXIL).is_none(), "ACO code, no DXIL part");
    let accc = c.part(FourCC::ACCC).expect("an ACCC part").data;
    assert_eq!(&accc[..4], b"FSPX");

    // 2. The FSPX program extraction succeeds: one compute program, sane
    //    register counts, non-empty code.
    let programs = container_programs(&c).expect("the ACCC program parses");
    assert_eq!(programs.len(), 1);
    let p = &programs[0];
    assert!(!p.code.is_empty());
    assert!(p.vgprs > 0 && p.vgprs <= 256);

    // 3. Every instruction reachable from offset 0 decodes without panics.
    //    (Decode walks the control flow; a panic here would mean the ACO
    //    encodings broke the decoder's bounds or flow assumptions.)
    let insts = victory_shader::gfx10::decode(&p.code).expect("RDNA code decodes");
    assert!(!insts.is_empty());

    // 4. Full translation may still refuse the scalar-lane-mask loop, but it
    //    must do so as a typed Unsupported/Isa error, never a panic and never
    //    a structural Malformed/NotDxbc (that would mean the ACCC port broke).
    if let Err(e) = victory_shader::emit::translate(BLOB) {
        assert!(
            matches!(e, ShaderError::Unsupported { .. } | ShaderError::Isa { .. }),
            "unexpected error kind for the ACO upscaler: {e}"
        );
    }
}