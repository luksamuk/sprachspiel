use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ParamsNoDefault {
    command: String,
    count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ParamsWithDefault {
    command: String,
    #[serde(default)]
    count: Option<u32>,
}

#[test]
fn test_serde_option_no_default_with_json_null() {
    // This should parse fine - JSON null maps to Option::None
    let json = r#"{"command": "test", "count": null}"#;
    let result: Result<ParamsNoDefault, _> = serde_json::from_str(json);
    assert!(result.is_ok());
    let params = result.unwrap();
    assert_eq!(params.command, "test");
    assert!(params.count.is_none());
}

#[test]
fn test_serde_option_no_default_with_missing_field() {
    // This should parse fine - missing field maps to Option::None
    let json = r#"{"command": "test"}"#;
    let result: Result<ParamsNoDefault, _> = serde_json::from_str(json);
    assert!(result.is_ok());
    let params = result.unwrap();
    assert_eq!(params.command, "test");
    assert!(params.count.is_none());
}

#[test]
fn test_serde_option_no_default_with_value() {
    let json = r#"{"command": "test", "count": 5}"#;
    let result: Result<ParamsNoDefault, _> = serde_json::from_str(json);
    assert!(result.is_ok());
    let params = result.unwrap();
    assert_eq!(params.command, "test");
    assert_eq!(params.count, Some(5));
}

#[test]
fn test_serde_option_no_default_with_string_null() {
    // This should FAIL - "null" string cannot be parsed as u32
    let json = r#"{"command": "test", "count": "null"}"#;
    let result: Result<ParamsNoDefault, _> = serde_json::from_str(json);
    println!("String 'null' result: {:?}", result);
    assert!(result.is_err(), "String 'null' should fail for Option<u32>");
}

#[test]
fn test_serde_option_no_default_with_string_number() {
    // This FAILS - serde_json strict typing requires number, not string
    let json = r#"{"command": "test", "count": "5"}"#;
    let result: Result<ParamsNoDefault, _> = serde_json::from_str(json);
    println!("String '5' result: {:?}", result);
    // serde_json is strict: "5" string is NOT valid for u32, must be raw number 5
    assert!(
        result.is_err(),
        "String '5' should NOT work for Option<u32>"
    );
}

#[test]
fn test_serde_option_empty_string_for_number() {
    // Empty string "" should FAIL for Option<u32>
    let json = r#"{"command": "test", "count": ""}"#;
    let result: Result<ParamsNoDefault, _> = serde_json::from_str(json);
    println!("Empty string result: {:?}", result);
    assert!(result.is_err(), "Empty string should fail for Option<u32>");
}

#[test]
fn test_serde_option_with_default_json_null() {
    let json = r#"{"command": "test", "count": null}"#;
    let result: Result<ParamsWithDefault, _> = serde_json::from_str(json);
    assert!(result.is_ok());
    let params = result.unwrap();
    assert_eq!(params.command, "test");
    assert!(params.count.is_none());
}

#[test]
fn test_serde_option_with_default_missing_field() {
    let json = r#"{"command": "test"}"#;
    let result: Result<ParamsWithDefault, _> = serde_json::from_str(json);
    assert!(result.is_ok());
    let params = result.unwrap();
    assert_eq!(params.command, "test");
    assert!(params.count.is_none());
}

#[test]
fn test_serde_option_with_default_string_null() {
    // Even with #[serde(default)], "null" as string still fails for Option<u32>
    let json = r#"{"command": "test", "count": "null"}"#;
    let result: Result<ParamsWithDefault, _> = serde_json::from_str(json);
    // This should STILL fail - "null" string cannot be parsed as u32
    assert!(result.is_err());
}
