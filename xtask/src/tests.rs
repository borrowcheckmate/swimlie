use super::*;
use crate::devices::{Device, is_uuid, parse_devices, resolve_phone};
use crate::frames::{parse_frames, sheet_shape};
use crate::recorder::swim_laps;

fn dev(name: &str, id: &str, paired: bool) -> Device {
    let state = if paired {
        "available (paired)"
    } else {
        "connected"
    };
    Device {
        name: name.into(),
        id: id.into(),
        state: state.into(),
        paired,
    }
}

#[test]
fn frames_shape_and_args() {
    assert_eq!(sheet_shape(14.86, 1.0, 5), (15, 3));
    assert_eq!(sheet_shape(0.2, 1.0, 5), (1, 1));
    assert_eq!(sheet_shape(10.0, 2.0, 4), (20, 5));
    let args: Vec<String> = ["v.mp4", "--fps", "2", "--cols", "3"]
        .map(String::from)
        .to_vec();
    let o = parse_frames(&args).unwrap();
    assert_eq!(
        (o.video.as_str(), o.fps, o.cols, o.out),
        ("v.mp4", 2.0, 3, None)
    );
    assert!(parse_frames(&["v.mp4".into(), "--fps".into(), "0".into()]).is_err());
}

#[test]
fn dotenv_lines() {
    let text = "# comment\nSWIMLIE_PHONE = iPhone 17 \n\nA=\"two words\"\nB='x'\nC=\"open\nD=\n";
    let got = parse_dotenv(text);
    let pair = |k: &str, v: &str| (k.to_string(), v.to_string());
    assert_eq!(
        got,
        vec![
            pair("SWIMLIE_PHONE", "iPhone 17"),
            pair("A", "two words"),
            pair("B", "x"),
            pair("C", "\"open"),
            pair("D", ""),
        ]
    );
}

#[test]
fn phone_resolution() {
    let devices = [
        dev("iPhone 17", "AAA", true),
        dev("iPhone 18", "BBB", true),
        dev("Watch", "CCC", false),
    ];
    // --phone beats SWIMLIE_PHONE, which beats the paired-device rule.
    assert_eq!(
        resolve_phone(Some("iphone 18"), Some("AAA"), &devices)
            .unwrap()
            .id,
        "BBB"
    );
    assert_eq!(
        resolve_phone(None, Some("AAA"), &devices).unwrap().id,
        "AAA"
    );
    assert!(resolve_phone(Some("nope"), Some("AAA"), &devices).is_err());
    // Two paired phones and no choice is an error; one paired phone is picked.
    assert!(resolve_phone(None, None, &devices).is_err());
    let one = [dev("iPhone 17", "AAA", true), dev("Watch", "CCC", false)];
    assert_eq!(resolve_phone(None, None, &one).unwrap().id, "AAA");
}

// Expected output is what scripts/swim-laps.py printed for this sample.
#[test]
fn laps_match_the_old_script() {
    let sample = r#"{"kind":"clock","wall":1000.0}
{"kind":"event","type_name":"pause","start":1010.4,"end":1010.4}
{"kind":"event","type_name":"lap","start":1020.0,"end":1045.5,"stroke_style_name":"freestyle","metadata":{"HKSWOLFScore":"41.0"},"received_at":1046.0}
{"kind":"motion","x":1}
{"kind":"event","type_name":"segment","start":1025.0,"end":1026.0,"stroke_style_name":"x","metadata":{},"received_at":1026.0}
{"kind":"event","type_name":"lap","start":1060.0,"end":1085.25,"stroke_style_name":"backstroke","metadata":{"HKSWOLFScore":38},"received_at":1086.5}
{"kind":"event","type_name":"lap","start":1090.0,"end":1110.0,"stroke_style_name":"breaststroke","metadata":{"HKSWOLFScore":35.5},"received_at":1110.2}
{"kind":"event","type_name":"lap","start":1104.0,"end":1106.0,"stroke_style_name":"butterfly","metadata":{"HKSWOLFScore":0},"received_at":1106.1}
{"kind":"event","type_name":"resume","start":1300.6,"end":1300.6}
"#;
    let expected = "  1      20  25.5s freest swolf   41 delay   0.5
   --- rest    14s
  2      60  25.2s backst swolf   38 delay   1.2
  3      90  20.0s breast swolf   36 delay   0.2
  4     104   2.0s butter swolf    0 delay   0.1
pause 10
resume 301
";
    assert_eq!(swim_laps(sample).unwrap(), expected);
}

#[test]
fn laps_print_the_planned_workout_and_old_files_still_read() {
    let line = r#"{"kind":"workout","id":"w1","workout":{"name":"Easy","pool_length_m":25,"blocks":[{"sets":[{"reps":4}]}]}}"#;
    let out = swim_laps(line).unwrap();
    assert!(
        out.starts_with("planned: Easy (id w1, pool 25 m)\n  {\"reps\":4}\n"),
        "{out}"
    );
    assert_eq!(swim_laps("{\"kind\":\"clock\",\"wall\":1.0}").unwrap(), "");
}

#[test]
fn is_uuid_accepts_simulator_and_hardware_ids_only() {
    assert!(is_uuid("CF1B4B17-942E-49DC-ACE5-6FC23D196F52"));
    assert!(is_uuid("00008130-0000000000000000"));
    assert!(!is_uuid("iPhone van Test"));
    assert!(!is_uuid("0000813-001A09A93C32001C"));
}

#[test]
fn parse_devices_reads_watch_and_simulator_lines() {
    let text = "Name  Hostname  Identifier  State  Model
Apple Watch van Test                    00008301-0000000000000000 (UDID)              available (paired)   Watch6,15
iPhone 18 Pro                 2AC5F98B-4457-4C29-BEF8-682B6C2A0BBA (UDID)   shutdown             iPhone 18 Pro (iPhone19,2)                simulated
";
    let devices = parse_devices(text);
    assert_eq!(devices.len(), 2);
    assert_eq!(devices[0].name, "Apple Watch van Test");
    assert_eq!(devices[0].id, "00008301-0000000000000000");
    assert_eq!(devices[0].state, "available (paired)");
    assert!(devices[0].paired);
    assert_eq!(devices[1].name, "iPhone 18 Pro");
    assert_eq!(devices[1].id, "2AC5F98B-4457-4C29-BEF8-682B6C2A0BBA");
    assert_eq!(devices[1].state, "shutdown");
    assert!(!devices[1].paired);
}
