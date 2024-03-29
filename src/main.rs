#![feature(rustc_private)]
#![feature(assert_matches)]
#![feature(control_flow_enum)]
#![feature(entry_insert)]
#![feature(hash_set_entry)]

extern crate rustc_middle;
#[macro_use]
extern crate rustc_smir;
extern crate rustc_driver;
extern crate rustc_interface;
extern crate stable_mir;

use diagnostics::create_error;
use mir::TerminatorKind::*;
use rustc_middle::ty::TyCtxt;
use rustc_smir::rustc_internal;
use stable_mir::*;
use std::io::Write;
use std::ops::ControlFlow;

mod diagnostics;

const CRATE_NAME: &str = "input";

/// This function uses the Stable MIR APIs to get information about the test crate.
fn test_stable_mir(_tcx: TyCtxt<'_>) -> ControlFlow<()> {
    let items = stable_mir::all_local_items();

    // Get all items and split generic vs monomorphic items.
    let (generic, mono): (Vec<_>, Vec<_>) = items
        .into_iter()
        .partition(|item| item.requires_monomorphization());
    assert_eq!(mono.len(), 3, "Expected 2 mono functions and one constant");
    assert_eq!(generic.len(), 2, "Expected 2 generic functions");

    // For all monomorphic items, get the correspondent instances.
    let instances = mono
        .iter()
        .filter_map(|item| mir::mono::Instance::try_from(*item).ok())
        .collect::<Vec<mir::mono::Instance>>();
    assert_eq!(instances.len(), mono.len());

    // For all generic items, try_from should fail.
    assert!(generic
        .iter()
        .all(|item| mir::mono::Instance::try_from(*item).is_err()));

    for instance in instances {
        test_body(instance.body().unwrap())
    }
    ControlFlow::Continue(())
}

/// Inspect the instance body
fn test_body(body: mir::Body) {
    for term in body.blocks.iter().map(|bb| &bb.terminator) {
        match &term.kind {
            Call { func, .. } => {
                let name = func.ty(&body.locals()).unwrap().kind();
                let name = name.rigid().unwrap();
                let stable_mir::ty::RigidTy::FnDef(def, _) = name else {
                    unreachable!()
                };
                let name = def.name();
                create_error(&[(term.span, Some("call here"))], &name);
            }
            Goto { .. } | Assert { .. } | SwitchInt { .. } | Return | Drop { .. } => {
                /* Do nothing */
            }
            _ => {
                unreachable!("Unexpected terminator {term:?}")
            }
        }
    }
}

/// This test will generate and analyze a dummy crate using the stable mir.
/// For that, it will first write the dummy crate into a file.
/// Then it will create a `StableMir` using custom arguments and then
/// it will run the compiler.
fn main() {
    let path = "instance_input.rs";
    generate_input(&path).unwrap();
    let args = vec![
        "rustc".to_string(),
        "-Cpanic=abort".to_string(),
        "--crate-type=lib".to_string(),
        "--crate-name".to_string(),
        CRATE_NAME.to_string(),
        path.to_string(),
    ];
    run!(args, tcx, test_stable_mir(tcx)).unwrap();
}

fn generate_input(path: &str) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    write!(
        file,
        r#"
    pub fn ty_param<T>(t: &T) -> T where T: Clone {{
        t.clone()
    }}

    pub fn const_param<const LEN: usize>(a: [bool; LEN]) -> bool {{
        LEN > 0 && a[0]
    }}

    extern "C" {{
        // Body should not be available.
        fn setpwent();
    }}

    pub fn monomorphic() {{
        let v = vec![10];
        let dup = ty_param(&v);
        assert_eq!(v, dup);
        unsafe {{ setpwent() }};
    }}

    pub mod foo {{
        pub fn bar_mono(i: i32) -> i64 {{
            i as i64
        }}
    }}
    "#
    )?;
    Ok(())
}
