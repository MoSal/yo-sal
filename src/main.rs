#[macro_use]
extern crate json;

use std::process::Command;
use std::env;
use json::JsonValue;

fn main() {
    let url = env::args().nth(1).unwrap();
    println!("Getting info from youtube-dl...");
    let cmd = Command::new("youtube-dl")
        .arg("--dump-json")
        .arg(url)
        .output()
        .unwrap();

    if !cmd.status.success() {
        panic!("Getting JSON failed. STDERR:\n{}",
               String::from_utf8_lossy(&cmd.stderr));
    }

    println!("Parsing JSON...");
    let yt_json = json::parse(&String::from_utf8_lossy(&cmd.stdout)).unwrap();
    let fname = format!("{}-{}", yt_json["fulltitle"], yt_json["id"]);
    if let JsonValue::Array(ref formats) = yt_json["formats"] {
        let info = formats.iter()
            .filter(|fmt| fmt["protocol"].as_str().unwrap().starts_with("http"))
            .last()
            .unwrap();

        let mut args = vec!["-nTrm".into(), "-a2".into()];
        args.push(format!("{}", info["url"]));
        args.push("-o".into());
        args.push(format!("{}.{}", fname, info["ext"]).replace(" ", "_"));

        if let JsonValue::Object(ref hdrs_json) = info["http_headers"] {
            for (k, v) in hdrs_json.iter() {
                args.push("--custom-headers".into());
                args.push(format!("{}: {}", k, v));
            }
        }
        println!("Running saldl with args: {:?}\n==========\n", args);
        let _ = Command::new("saldl")
            .args(&args)
            .status()
            .unwrap();
    } else {
        panic!("No formats array.");
    }

}
