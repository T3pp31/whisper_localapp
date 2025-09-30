use whisper_realtime_api::config::ConfigSet;

#[test]
fn server_ws_port_is_8081() {
    let cfg = ConfigSet::load_from_dir("config").expect("load config");
    assert!(cfg.server.ws_bind_addr.ends_with(":8081"));
}

