// Teksty Setup z ../i18n (shrimp_i18n::codegen) i lista języków GNOME z boxes/i18n-core/langs.ts
// (jedno źródło prawdy z Shrimp Install): (locale, znacznik tłumaczeń, nazwa w tym języku, pismo od prawej).
use std::{env, fs, path::Path};

fn main() {
    shrimp_i18n::codegen::write("../i18n");
    let langs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../i18n-core/langs.ts");
    println!("cargo:rerun-if-changed={}", langs.display());
    let ts = fs::read_to_string(&langs).expect("boxes/i18n-core/langs.ts");
    // ROWS: [tag, nazwa, flagi, ...]; „r” we flagach = pismo od prawej do lewej.
    let rows: Vec<(String, String, bool)> = ts_rows(&ts, "const ROWS: Row[] = [")
        .iter()
        .filter_map(|r| {
            let q = quoted(r);
            (q.len() >= 3).then(|| (q[0].clone(), q[1].clone(), q[2].contains('r')))
        })
        .collect();
    let mut out = String::from("pub static GNOME_LANGS: &[(&str, &str, &str, bool)] = &[\n");
    for r in ts_rows(&ts, "export const GNOME_LOCALES: [locale: string, tag: string, name?: string][] = [") {
        let q = quoted(&r);
        if q.len() < 2 {
            continue;
        }
        let row = rows.iter().find(|x| x.0 == q[1]);
        let name = q.get(2).cloned().or_else(|| row.map(|x| x.1.clone())).unwrap_or_else(|| q[1].clone());
        out += &format!("    ({:?}, {:?}, {:?}, {}),\n", format!("{}.UTF-8", q[0]), q[1], name, row.is_some_and(|x| x.2));
    }
    out += "];\n";
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("langs.rs"), out).unwrap();
}

/// Wiersze `[...]` tablicy zaczynającej się po `start` (jak w boxes/shrimp-i18n/build.rs).
fn ts_rows(ts: &str, start: &str) -> Vec<String> {
    let Some(i) = ts.find(start) else { return vec![] };
    let (mut rows, mut cur, mut depth, mut in_str) = (vec![], String::new(), 0, false);
    for line in ts[i + start.len()..].lines() {
        if depth == 0 && line.trim_start().starts_with("//") {
            continue;
        }
        if depth == 0 && line.starts_with("];") {
            break;
        }
        for c in line.chars().chain(std::iter::once(' ')) {
            if c == '"' {
                in_str = !in_str;
            }
            if !in_str && c == '[' {
                depth += 1;
                if depth == 1 {
                    continue;
                }
            }
            if !in_str && c == ']' {
                depth -= 1;
                if depth == 0 {
                    rows.push(std::mem::take(&mut cur));
                    continue;
                }
            }
            if depth > 0 {
                cur.push(c);
            }
        }
    }
    rows
}

fn quoted(s: &str) -> Vec<String> {
    s.split('"').skip(1).step_by(2).map(String::from).collect()
}
