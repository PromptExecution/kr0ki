//! Live test against a real SysMD sidecar. Ignored by default: `just test-live-sysmd` (starts the image) or
//! `KR0KI_SYSMD_URL=http://127.0.0.1:8081 cargo test -p kr0ki-core --test sysmd_live -- --ignored`.

use kr0ki_core::sysmd_client::{
    SolveVerdict, SolvedVariable, SysmdClient, SysmdLanguage, ValueRange,
};

fn client() -> SysmdClient {
    let url =
        std::env::var("KR0KI_SYSMD_URL").expect("set KR0KI_SYSMD_URL to run the live SysMD test");
    SysmdClient::new(url)
}

fn var<'a>(vars: &'a [SolvedVariable], path: &str) -> &'a SolvedVariable {
    vars.iter().find(|v| v.path == path).unwrap_or_else(|| {
        panic!(
            "no variable {path}; have {:?}",
            vars.iter().map(|v| &v.path).collect::<Vec<_>>()
        )
    })
}

fn contains(v: &SolvedVariable, truth: f64) -> bool {
    match &v.range {
        ValueRange::Bounded { range } => range.lo() <= truth && truth <= range.hi(),
        _ => false,
    }
}

#[tokio::test]
#[ignore = "needs a SysMD sidecar (KR0KI_SYSMD_URL)"]
async fn solves_a_model_with_units_and_the_ranges_contain_the_truth() {
    let c = client();
    c.ping().await.unwrap();
    let r = c
        .solve(
            "attribute m: ISQ::MassValue = 2.0 [kg]; attribute n: ISQ::MassValue = m * 3.0; \
             attribute l: ISQ::LengthValue = 0.1 .. 0.3 [m]; attribute w: ISQ::LengthValue = l * 2.0; \
             attribute q: ScalarValues::Real = 1.0 / 3.0; attribute a: ScalarValues::Real = 2.000001; \
             attribute z: ScalarValues::Real = 0.0 .. 0.0000001; attribute mm: ISQ::LengthValue = 5.0 [mm]; \
             attribute area: ISQ::AreaValue = l * l; attribute f: ISQ::ForceValue = 4.0 [N]; \
             attribute big: ISQ::MassValue = 123456.789 [kg];",
            SysmdLanguage::Sysml,
        )
        .await
        .unwrap();
    assert_eq!(r.verdict, SolveVerdict::Consistent, "{:?}", r.issues);

    let n = var(&r.variables, "n");
    assert!(contains(n, 6.0), "{n:?}");
    assert_eq!(n.quantity.as_ref().unwrap().unit.dimension.mass, 1);
    let w = var(&r.variables, "w");
    assert!(contains(w, 0.2) && contains(w, 0.6), "{w:?}");
    assert_eq!(w.quantity.as_ref().unwrap().unit.dimension.length, 1);
    assert!(contains(var(&r.variables, "q"), 1.0 / 3.0));
    // SysMD printed `2` for 2.000001 and `0` for 0..1e-7: the widened intervals must still hold the truth
    assert!(contains(var(&r.variables, "a"), 2.000001));
    let z = var(&r.variables, "z");
    assert!(contains(z, 0.0) && contains(z, 1e-7), "{z:?}");
    // 5 mm comes back in the SI base unit
    assert!(contains(var(&r.variables, "mm"), 0.005));
    assert!(contains(var(&r.variables, "big"), 123456.789));
    // compound units are mapped by dimension
    let area = var(&r.variables, "area");
    assert_eq!(
        area.quantity.as_ref().unwrap().unit.dimension.length,
        2,
        "{area:?}"
    );
    let f = var(&r.variables, "f");
    let u = &f.quantity.as_ref().unwrap().unit;
    assert_eq!(
        (u.dimension.mass, u.dimension.length, u.dimension.time),
        (1, 1, -2),
        "{f:?}"
    );
    // none of the standard library leaks into the answer
    assert!(r.variables.iter().all(|v| !v.path.starts_with("Base::")
        && !v.path.contains("multiplicity")
        && !v.path.starts_with("ISQ::")));
}

#[tokio::test]
#[ignore = "needs a SysMD sidecar (KR0KI_SYSMD_URL)"]
async fn a_syntax_error_is_an_error_verdict_and_an_unsatisfiable_model_is_inconsistent() {
    let c = client();
    let bad = c.solve("part x {{{", SysmdLanguage::Sysml).await.unwrap();
    assert_eq!(bad.verdict, SolveVerdict::Error, "{:?}", bad.issues);
    assert!(!bad.issues.is_empty());

    let r = c
        .solve(
            "attribute u: ScalarValues::Real; attribute v: ScalarValues::Real = u / 0.0;",
            SysmdLanguage::Sysml,
        )
        .await
        .unwrap();
    assert_eq!(r.verdict, SolveVerdict::Inconsistent, "{:?}", r.issues);
}

#[tokio::test]
#[ignore = "needs a SysMD sidecar (KR0KI_SYSMD_URL)"]
async fn nested_parts_report_their_paths_and_sessions_are_released() {
    let c = client();
    let r = c
        .solve(
            "part def B { attribute mass: ISQ::MassValue; } part b: B { :>> mass = 2.0 [kg]; part inner: B { :>> mass = 1.5 [kg]; } }",
            SysmdLanguage::Sysml,
        )
        .await
        .unwrap();
    assert_eq!(r.verdict, SolveVerdict::Consistent, "{:?}", r.issues);
    assert!(contains(var(&r.variables, "b::mass"), 2.0));
    assert!(contains(var(&r.variables, "b::inner::mass"), 1.5));
    // every solve deletes its session: none are left behind
    let base = std::env::var("KR0KI_SYSMD_URL").unwrap();
    let sessions: serde_json::Value = reqwest::get(format!("{base}/session"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        sessions.as_array().unwrap().len(),
        0,
        "leaked sessions: {sessions}"
    );
}
