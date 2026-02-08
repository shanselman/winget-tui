// Quick integration test for the parser
// Run with: cargo test --test parse_test -- --nocapture

#[tokio::test]
async fn test_winget_list_parsing() {
    let output = tokio::process::Command::new("winget")
        .args(&["list", "--accept-source-agreements", "--disable-interactivity"])
        .output()
        .await
        .expect("winget not found");

    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    // Resolve \r progress overwrites (same as CliBackend::run_winget)
    let stdout: String = raw
        .replace("\r\n", "\n")
        .split('\n')
        .map(|line| {
            if line.contains('\r') {
                line.rsplit('\r').next().unwrap_or(line)
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let lines: Vec<&str> = stdout.lines().collect();
    println!("Total lines: {}", lines.len());
    for (i, l) in lines.iter().enumerate().take(15) {
        let trimmed = l.trim();
        let is_sep = trimmed.len() > 10
            && trimmed.chars().all(|c| c == '-' || c == ' ')
            && trimmed.contains('-');
        println!("  [{i}] len={:3} sep={is_sep} | {:?}", l.len(), &l[..l.len().min(90)]);
    }

    let sep_idx = lines.iter().position(|l| {
        let trimmed = l.trim();
        trimmed.len() > 10
            && trimmed.chars().all(|c| c == '-' || c == ' ')
            && trimmed.contains('-')
    });

    let sep_idx = sep_idx.expect("No separator found");
    assert!(sep_idx > 0, "Separator at index 0");

    let header = lines[sep_idx - 1];
    println!("Header: {:?}", header);
    println!("Separator at line: {}", sep_idx);
    assert!(header.contains("Name"), "Header should contain 'Name'");
    assert!(header.contains("Id"), "Header should contain 'Id'");

    let data_lines: Vec<&&str> = lines[sep_idx + 1..]
        .iter()
        .filter(|l| !l.trim().is_empty())
        .take_while(|l| l.len() > 20 || !l.trim_start().starts_with(|c: char| c.is_ascii_digit()))
        .collect();

    println!("Data lines: {}", data_lines.len());
    assert!(data_lines.len() > 0, "No data lines found after separator");

    for l in data_lines.iter().take(3) {
        println!("  {:?}", &l[..l.len().min(80)]);
    }
}
