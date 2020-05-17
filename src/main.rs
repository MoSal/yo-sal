use std::process::Command;
use std::env;
//use json::JsonValue;
use serde_json::Value as JsonValue;

fn get_file(yt_json: &JsonValue, name: Option<String>) {
    let id = yt_json["id"].as_str().expect("id exists");
    let title = yt_json["fulltitle"].as_str().or_else(|| yt_json["title"].as_str()).expect("fulltitle or title exists");
    let ext = yt_json["ext"].as_str().expect("ext exists");

    let fname = if let Some(formatted_name) = name {
        formatted_name
            .replace("%t", &unescape::unescape(title).expect("failed to unescape"))
            .replace("%i", id)
            .replace("%e", ext)
    } else {
        format!("{}-{}.{}",
                id,
                unescape::unescape(title).expect("failed to unescape"),
                ext)
        .replace(" ", "_")
    };

    let info = if let JsonValue::Array(ref formats) = yt_json["formats"] {
        formats.iter()
            .filter(|fmt| fmt["protocol"].as_str().expect("protocol exists").starts_with("http") || fmt["protocol"].as_str().expect("protocol exists") == "")
            .last()
            .expect("atleast one format exists")
    } else {
        // generic, no formats
        &yt_json
    };

    let mut args = vec!["-nTrm".into(), "-a2".into()];
    args.push(format!("{}", info["url"]).trim_matches('"').to_string());
    args.push("-o".into());
    args.push(fname);

    if let JsonValue::Object(ref hdrs_json) = info["http_headers"] {
        for (k, v) in hdrs_json.iter() {
            args.push("-H".into());
            args.push(format!("{}: {}", k, v));
        }
    }
    println!("Running saldl with args: {:?}\n==========\n", args);
    let _ = Command::new("saldl")
        .args(&args)
        .status()
        .expect("successful status");
}

fn main() {
    let url = env::args().nth(1).expect("url argument provided");

    let info_bytes = if url.starts_with("/") {
        std::fs::read(url).unwrap()
    } else {
        println!("Getting info from youtube-dl...");
        let cmd = Command::new("youtube-dl")
            .arg("--no-playlist")
            .arg("-J")
            .arg(url)
            .output()
            .unwrap();

        if !cmd.status.success() {
            panic!("Getting JSON failed. STDERR:\n{}",
                   String::from_utf8_lossy(&cmd.stderr));
        }
        cmd.stdout
    };


    println!("Parsing JSON...");
    //let yt_json = json::parse(&String::from_utf8_lossy(&info_bytes)).expect("successful JSON parse");
    let yt_json: JsonValue = serde_json::from_slice(&*info_bytes).expect("successful JSON parse");

    // If even with --no-playlist, we still get a playlist. Then maybe we should grab everything
    //if yt_json["_type"].as_str().expect("string _type value") == "playlist" {
    if let Some("playlist") = yt_json["_type"].as_str() {
        // Dedup by url
        //let mut entries = yt_json["entries"].members().collect::<Vec<_>>();
        let mut entries = yt_json["entries"].as_array().cloned().expect("not array");
        let pre_dedup_len = entries.len();
        println!("[warning] multiple entries {} in an unexpected playlist.", pre_dedup_len);
        entries.sort_by_key(|e| e["url"].as_str().unwrap().to_owned());
        entries.dedup_by_key(|e| e["url"].as_str().unwrap().to_owned());
        let post_dedup_len = entries.len();
        if pre_dedup_len != post_dedup_len {
            println!("[warning] Remaining entries after deduplication by url: {}/{}.", pre_dedup_len, post_dedup_len);
        }
        if entries.len() > 1 {
            // download all
            for (idx, entry) in entries.iter().enumerate() {
                let name_opt = env::args().nth(2).map(|mut s| {
                    let name_suffix = format!("-{:03}", idx+1);
                    let ext_period_pos = s.rfind('.').expect("ext in name arg");
                    s.replace_range(ext_period_pos..ext_period_pos, &name_suffix);
                    s
                });
                get_file(entry, name_opt);
            }
        } else {
            get_file(&yt_json["entries"][0], env::args().nth(2));
        }
    } else {
        get_file(&yt_json, env::args().nth(2));
    }

}
