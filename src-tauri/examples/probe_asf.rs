// 验证:同一 blocking Client 连接池上,m3u8 大响应后紧跟分片请求是否 502
fn main() {
    let m3u8_url = std::env::args().nth(1).expect("m3u8 url");
    let seg_url = std::env::args().nth(2).expect("seg url");
    let client = reqwest::blocking::Client::builder()
        .user_agent(
            "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 Chrome/120 Mobile Safari/537.36",
        )
        .build()
        .unwrap();
    let r1 = client
        .get(&m3u8_url)
        .header("Referer", "https://fs1.app/")
        .send()
        .unwrap();
    let b1 = r1.text().unwrap();
    println!("m3u8: {} len={}", "200", b1.len());

    let r2 = client
        .get(&seg_url)
        .header("Referer", "https://fs1.app/")
        .send()
        .unwrap();
    let b2 = r2.text().unwrap();
    println!("seg(同client): {} len={}", "200", b2.len());

    let c2 = reqwest::blocking::Client::builder().build().unwrap();
    let r3 = c2
        .get(&seg_url)
        .header("Referer", "https://fs1.app/")
        .send()
        .unwrap();
    let b3 = r3.text().unwrap();
    println!("seg(新client): {} len={}", "200", b3.len());
}
