#[test]
fn version_coincide_con_cargo() {
    assert_eq!(gestor_core::version(), env!("CARGO_PKG_VERSION"));
}
