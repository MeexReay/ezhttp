use ezhttp::request::IntoURL;

#[test]
fn url_full() {
    let url = "https://meex.lol:456/dku?key=value&key2=value2#hex_id".to_url().unwrap();
    let root = url.clone().root.unwrap();
    assert_eq!(root.domain, "meex.lol");
    assert_eq!(root.port, 456);
    assert_eq!(root.scheme, "https");
    assert_eq!(url.anchor.clone(), Some("hex_id".to_string()));
    assert_eq!(url.path.clone(), "/dku");
    assert_eq!(url.query.get("key"), Some(&"value".to_string()));
    assert_eq!(url.query.get("key2"), Some(&"value2".to_string()));
    assert_eq!(url.query.len(), 2);
    assert!(
        url.to_string() == "https://meex.lol:456/dku?key=value&key2=value2#hex_id" || 
        url.to_string() == "https://meex.lol:456/dku?key2=value2&key=value#hex_id"
    );
}

#[test]
fn url_path() {
    let url = "/dku?key=value&key2=value2#hex_id".to_url().unwrap();
    assert!(url.root.is_none());
    assert_eq!(url.anchor.clone(), Some("hex_id".to_string()));
    assert_eq!(url.path.clone(), "/dku");
    assert_eq!(url.query.get("key"), Some(&"value".to_string()));
    assert_eq!(url.query.get("key2"), Some(&"value2".to_string()));
    assert_eq!(url.query.len(), 2);
    assert_eq!(url.query.len(), 2);
    assert!(
        url.to_string() == "/dku?key=value&key2=value2#hex_id" || 
        url.to_string() == "/dku?key2=value2&key=value#hex_id"
    );
}