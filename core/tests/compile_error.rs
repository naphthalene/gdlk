//! Integration tests for GDLK that expect compile errors. The programs in
//! these tests should all fail during compilation.

use gdlk::{compile, HardwareSpec, ProgramSpec};

/// Compiles the program for the given hardware, expecting compile error(s).
/// Panics if the program compiles successfully, or if the wrong set of
/// errors is returned.
fn expect_compile_errors(
    env: HardwareSpec,
    src: &str,
    expected_errors: &[&str],
) {
    // Compile from hardware+src
    let actual_errors = compile(
        &env,
        // This won't be used, just use bullshit values
        &ProgramSpec {
            input: vec![],
            expected_output: vec![],
        },
        src.into(),
    )
    .unwrap_err();
    assert_eq!(format!("{}", actual_errors), expected_errors.join("\n"));
}

#[test]
fn test_parse_empty_file() {
    expect_compile_errors(
        HardwareSpec {
            num_registers: 1,
            num_stacks: 0,
            max_stack_length: 0,
        },
        "",
        &["Parse error: 0: in Alpha, got empty input\n\n"],
    );
}

#[test]
fn test_parse_no_newline_after_inst() {
    // TODO: make this error nicer
    expect_compile_errors(
        HardwareSpec {
            num_registers: 1,
            num_stacks: 0,
            max_stack_length: 0,
        },
        "READ RX1 WRITE RX2",
        &["Parse error: Invalid keyword:  WRITE RX2"],
    );
}

#[test]
fn test_invalid_user_reg_ref() {
    expect_compile_errors(
        HardwareSpec {
            num_registers: 1,
            num_stacks: 1,
            max_stack_length: 5,
        },
        "
        READ RX1
        WRITE RX2
        SET RX3 RX0
        ADD RX4 RX0
        SUB RX5 RX0
        MUL RX6 RX0
        PUSH RX7 S0
        POP S0 RX8
        ",
        &[
            "Invalid reference to register RX1",
            "Invalid reference to register RX2",
            "Invalid reference to register RX3",
            "Invalid reference to register RX4",
            "Invalid reference to register RX5",
            "Invalid reference to register RX6",
            "Invalid reference to register RX7",
            "Invalid reference to register RX8",
        ],
    );
}

#[test]
fn test_invalid_stack_reg_ref() {
    expect_compile_errors(
        HardwareSpec {
            num_registers: 1,
            num_stacks: 1,
            max_stack_length: 5,
        },
        "
        SET RX0 RS1
        ",
        &["Invalid reference to register RS1"],
    );
}

#[test]
fn test_invalid_stack_ref() {
    expect_compile_errors(
        HardwareSpec {
            num_registers: 1,
            num_stacks: 1,
            max_stack_length: 5,
        },
        "
        PUSH 5 S1
        POP S2 RX0
        ",
        &[
            "Invalid reference to stack S1",
            "Invalid reference to stack S2",
        ],
    );
}

#[test]
fn test_unwritable_reg() {
    expect_compile_errors(
        HardwareSpec {
            num_registers: 1,
            num_stacks: 1,
            max_stack_length: 5,
        },
        "
        SET RLI 5
        SET RS0 5
        ",
        &[
            "Cannot write to read-only register RLI",
            "Cannot write to read-only register RS0",
        ],
    );
}