use std::process::Command;
use std::env;
use serde_json::Value as JsonValue;

enum AV {
    AOnly,
    VOnly,
    Both,
}

fn get_file(yt_json: &JsonValue, name: Option<String>, av: AV) {
    let id = yt_json["id"].as_str().expect("id exists");
    let title = yt_json["fulltitle"].as_str().or_else(|| yt_json["title"].as_str()).expect("fulltitle or title exists");

    let info = if let JsonValue::Array(ref formats) = yt_json["formats"] {
        let a_only_filter = |fmt: &&serde_json::Value| {
            let a_ext = fmt["audio_ext"].as_str();
            a_ext != None && a_ext != Some("none")
        };
        let v_only_filter = |fmt: &&serde_json::Value| {
            let v_ext = fmt["video_ext"].as_str();
            v_ext != None && v_ext != Some("none")
        };
        let is_a_and_v_only_formats = {
            let a_only = formats.iter().filter(a_only_filter).count() > 0;
            let v_only = formats.iter().filter(v_only_filter).count() > 0;
            a_only && v_only
        };
        if env::var("YO_SAL_FMT").is_err() && matches!(av, AV::Both) && is_a_and_v_only_formats { // vid-only and aud-only formats
            get_file(yt_json, name.clone(), AV::AOnly);
            get_file(yt_json, name, AV::VOnly);
            return;
        }

        let proto = |fmt: &JsonValue| fmt["protocol"].as_str().expect("protocol exists").to_owned();
        let fmt_id = |fmt: &JsonValue| fmt["format_id"].as_str().expect("format_id exists").to_owned();
        if let Ok(forced_fmt_id) = env::var("YO_SAL_FMT") {
            formats.iter()
                .filter(|fmt| fmt_id(fmt) == forced_fmt_id)
                .last()
                .expect("atleast one format exists")
        } else {
            let av_filtered_fmts = || formats.iter()
                .filter(|fmt| matches!(av, AV::AOnly).then(|| a_only_filter(fmt)).unwrap_or(true))
                .filter(|fmt| matches!(av, AV::VOnly).then(|| v_only_filter(fmt)).unwrap_or(true));

            // DASH streams might have http/https proto and mp4_dash container.
            // Try hls first
            av_filtered_fmts().filter(|fmt| proto(fmt).starts_with("m3u8")).last()
                .or_else(|| av_filtered_fmts().filter(|fmt| proto(fmt).starts_with("http") || proto(fmt) == "").last())
                .expect("atleast one format exists")
        }
    } else {
        // generic, no formats
        &yt_json
    };

    let ext = info["ext"].as_str().expect("ext exists");

    let mut title = unescape::unescape(title).expect("failed to unescape");
    title.truncate(128);

    let fname = if let Some(formatted_name) = name {
        formatted_name
            .replace("%t", &title)
            .replace("%i", id)
            .replace("%e", ext)
    } else {
        format!("{}-{}.{}",
                id,
                title,
                ext)
            // replace '/' too because we may not use saldl which usually takes care of this for us
            .replace(&[' ', '/'][..], "_")
    };

    let fname = match av {
        AV::AOnly => fname + "-aud",
        AV::VOnly => fname + "-vid",
        AV::Both => fname,
    };

    // Value adds quotes when turned to String (e.g. via format!() like below), so we strip the quotes
    let strip_quotes = |s: String| {
        let p = &['"', '\''][..];
        s.strip_prefix(p)
            .map(|s| s.strip_suffix(p).unwrap_or(&*s))
            .unwrap_or(&*s)
            .to_string()
    };

    let m3u8_cond = info["protocol"].as_str().map(|p| p.starts_with("m3u8")).unwrap_or(false);
    let dash_cond = info["container"].as_str().map(|c| c.contains("dash")).unwrap_or(false);

    if m3u8_cond {
        let mut args = Vec::with_capacity(16);

        if let JsonValue::Object(ref hdrs_json) = info["http_headers"] {
            for (k, v) in hdrs_json.iter() {
                args.push("-H".into());
                args.push(format!("{}: {}", k, strip_quotes(v.to_string())));
            }
        }

        args.push("--sub-seg-max-count=1".into());
        args.push("-p".into());
        args.push(format!("{}", info["url"]).trim_matches('"').to_string());
        args.push("-o".into());
        args.push(fname);
        println!("Running salgrab with args: {:?}\n==========\n", args);
        let _ = Command::new("salgrab")
            .args(&args)
            .status()
            .expect("successful status");
    } else if dash_cond {
        let mut args = vec!["--stream-segment-threads=2".into(), "--ringbuffer-size=100M".into()];

        if let JsonValue::Object(ref hdrs_json) = info["http_headers"] {
            for (k, v) in hdrs_json.iter() {
                args.push("--http-header".into());
                args.push(format!("{}={}", k, strip_quotes(v.to_string())));
            }
        }

        args.push(format!("{}", info["url"]).trim_matches('"').to_string());
        args.push("best".into());
        args.push("-o".into());
        args.push(fname);
        println!("Running streamlink with args: {:?}\n==========\n", args);
        let _ = Command::new("streamlink")
            .args(&args)
            .status()
            .expect("successful status");
    } else {
        let mut args = vec!["-nTrm".into(), "-a2".into()];
        args.push(format!("{}", info["url"]).trim_matches('"').to_string());
        args.push("-o".into());
        args.push(fname);

        if let JsonValue::Object(ref hdrs_json) = info["http_headers"] {
            for (k, v) in hdrs_json.iter() {
                args.push("-H".into());
                args.push(format!("{}: {}", k, strip_quotes(v.to_string())));
            }
        }

        println!("Running saldl with args: {:?}\n==========\n", args);
        let _ = Command::new("saldl")
            .args(&args)
            .status()
            .expect("successful status");
    }
}

fn main() {
    //const ytdl: &str = "youtube-dl";
    const YTDL: &str = "yt-dlp";
    let url = env::args().nth(1).expect("url argument provided");

    let info_bytes = if url.starts_with("/") {
        std::fs::read(url).unwrap()
    } else {
        println!("Getting info from {}...", YTDL);
        let cmd = Command::new(YTDL)
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
    let yt_json: JsonValue = serde_json::from_slice(&*info_bytes).expect("successful JSON parse");

    // If even with --no-playlist, we still get a playlist. Then maybe we should grab everything
    if let Some("playlist") = yt_json["_type"].as_str() {
        // Dedup by url
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
                get_file(entry, name_opt, AV::Both);
            }
        } else {
            get_file(&yt_json["entries"][0], env::args().nth(2), AV::Both);
        }
    } else {
        get_file(&yt_json, env::args().nth(2), AV::Both);
    }

}
