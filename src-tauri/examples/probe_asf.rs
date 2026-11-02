// asf-doc 域头组合实验(临时,用后即删)
fn main() {
    let url = std::env::args().nth(1).expect("url arg");
    let combos: Vec<(&str, Vec<(&str, &str)>)> = vec![
        ("Referer=fs1", vec![("Referer", "https://fs1.app/")]),
        ("Referer+Origin", vec![("Referer", "https://fs1.app/"), ("Origin", "https://fs1.app")]),
        ("Referer=missav", vec![("Referer", "https://missav.ai/")]),
        ("无 Referer", vec![]),
        (
            "完整浏览器头",
            vec![
                ("Referer", "https://fs1.app/"),
                ("Origin", "https://fs1.app"),
                ("User-Agent", "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 Chrome/120 Mobile Safari/537.36"),
                ("Accept", "*/*"),
                ("Accept-Language", "zh-CN,zh;q=0.9"),
            ],
        ),
    ];
    for (label, headers) in combos {
        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 Chrome/120 Mobile Safari/537.36")
            .build()
            .unwrap();
        let mut req = client.get(&url);
        for (k, v) in &headers {
            req = req.header(*k, *v);
        }
        match req.send() {
            Ok(r) => {
                let status = r.status();
                let len = r.content_length().unwrap_or(0);
                println!("{label}: {status} len={len}");
            }
            Err(e) => println!("{label}: ERR {e}"),
        }
    }
}
