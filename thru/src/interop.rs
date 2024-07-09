use reqwest::StatusCode;
use json::{object, JsonValue};

fn post_cmd(port: u16, data: JsonValue) {
    let client = reqwest::blocking::Client::new();
    match client.post(format!("http://localhost:{}", port))
                    .header("Content-type", "application/json")
                    .body(data.dump())
                    .send() {
        Err(e) => println!("{:?}", e),
        Ok(res) => {
            if res.status() != StatusCode::OK {
                println!("{:?} {:?}", data, res.status());
            }
        }
    }
}


pub fn post_cmd_to_recorder(data: JsonValue) {
    post_cmd(9009, data);
}

pub fn post_cmd_to_modulator(note: u8) {
    post_cmd(7878, object!{ note: note });
}

