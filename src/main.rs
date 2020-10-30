use error_chain::error_chain;
use std::io::Read;
use std::io::stdin;
use regex::Regex;
use log::{info, error, LevelFilter};
use clap::{Arg, App};
use m3u8_rs::playlist::{MediaPlaylist, MediaPlaylistType, MediaSegment};

error_chain! {
    foreign_links {
        Io(std::io::Error);
        HttpRequest(reqwest::Error);
    }
}

fn main() -> Result<()> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"));

    let matches = App::new("Twitch Playlist Fixer")
        .version("0.2")
        .author("vyneer <vyneer@protonmail.com>")
        .about("Fixes broken m3u8 twitch playlists.")
        .arg(Arg::with_name("input")
            .help("Sets the url to process."))
        .arg(Arg::with_name("old")
            .short("o")
            .help("Uses the old (slow, but more reliable) method of checking for segments."))
        .arg(Arg::with_name("v")
            .short("v")
            .help("Shows verbose info."))
        .get_matches();

    let re = Regex::new(r"[^/]+").unwrap();

    let mut url = String::new();

    let input_url = matches.value_of("input");

    if input_url == None {
        println!("Please input the m3u8 url.");

        stdin()
            .read_line(&mut url)
            .expect("Failed to read line.");
    } else {
        url = input_url.unwrap().to_string();

        if matches.is_present("v") {
            log::set_max_level(LevelFilter::Info);
        } else {
            log::set_max_level(LevelFilter::Warn);
        }
    }

    let state = match url.contains("twitch.tv") {
        false => {
            println!("This isn't a valid URL (need twitch.tv in URL).");
            false
        },
        true => true,
    };

    if state {
        let mut base_url_parts: Vec<String> = Vec::new();
        for elem in re.captures_iter(&url) {
            base_url_parts.push(elem[0].to_string());
        }
        let base_url = format!("https://vod-secure.twitch.tv/{}/{}/", base_url_parts[2], base_url_parts[3]);
    
        let mut res = reqwest::blocking::get(&url)?;
        let mut body = String::new();
        res.read_to_string(&mut body)?;
    
        let bytes = body.into_bytes();
    
        let mut playlist = MediaPlaylist { 
            ..Default::default()
        };

        match m3u8_rs::parse_media_playlist_res(&bytes) {
            Ok(pl) => {
                playlist = MediaPlaylist { 
                    version: pl.version,
                    target_duration: pl.target_duration,
                    media_sequence: pl.media_sequence,
                    discontinuity_sequence: pl.discontinuity_sequence,
                    end_list: pl.end_list,
                    playlist_type: Some(MediaPlaylistType::Vod),
                    ..Default::default()
                };
                if matches.is_present("old") {
                    for segment in pl.segments {
                        let url = format!("{}{}", base_url, segment.uri);
                        let res = reqwest::blocking::get(&url)?;
                        if res.status() == 403 {
                            let muted_url = format!("{}-muted.ts", &url.clone()[..url.len()-11]);
                            playlist.segments.push(MediaSegment {
                                uri: muted_url.clone(),
                                duration: segment.duration,
                                ..Default::default()
                            });
                            info!("Found the muted version of this .ts file - {:?}", muted_url)
                        } else if res.status() == 200 {
                            playlist.segments.push(MediaSegment {
                                uri: url.clone(),
                                duration: segment.duration,
                                ..Default::default()
                            });
                            info!("Found the unmuted version of this .ts file - {:?}", url)
                        }
                    }
                } else {
                    for segment in pl.segments {
                        let url = format!("{}{}", base_url, segment.uri);
                        if segment.uri.contains("unmuted") {
                            let muted_url = format!("{}-muted.ts", &url.clone()[..url.len()-11]);
                            playlist.segments.push(MediaSegment {
                                uri: muted_url.clone(),
                                duration: segment.duration,
                                ..Default::default()
                            });
                            info!("Found the muted version of this .ts file - {:?}", muted_url)
                        } else {
                            playlist.segments.push(MediaSegment {
                                uri: url.clone(),
                                duration: segment.duration,
                                ..Default::default()
                            });
                            info!("Found the unmuted version of this .ts file - {:?}", url)
                        }
                    }
                }
            },
            Err(e) => error!("Error: {:?}", e)
        }

        let mut file = std::fs::File::create(format!("muted_{}.m3u8", base_url_parts[2])).unwrap();
        playlist.write_to(&mut file).unwrap();
    }

    if input_url == None {
        println!("Press Enter to exit.");
        stdin().read_line(&mut String::new()).unwrap();
    }
    Ok(())
}