use std::{collections::HashSet, env, io::Write, path::Path};

fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=tests/comp");
    
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let mut output = std::fs::File::create(Path::new(&out_dir).join("compilation_tests.rs"))?;

    write_file_prelude(&mut output)?;

    let mut already_generated_tests = HashSet::new();

    for entry in std::fs::read_dir("tests/comp")? {
        let entry = entry?;
        if !entry.file_type()?.is_file() { continue }

        let file_name = entry.file_name().into_string().unwrap();
        let (test_name, _suffix) =  file_name.split_once('.').unwrap();
        // If we already came across this test, skip it
        if already_generated_tests.contains(test_name) { continue }
        
        let ir = std::fs::read_to_string(format!("tests/comp/{}.ir", test_name))?;
        let wat = std::fs::read_to_string(format!("tests/comp/{}.wat", test_name))?;
        generate_test_function(&mut output, test_name, &ir, &wat)?;

        already_generated_tests.insert(test_name.to_owned());
    }

    Ok(())
}

fn write_file_prelude(w: &mut impl Write) -> std::io::Result<()> {
    write!(w, r#"
extern crate swarm_ir;
extern crate wasmparser;

use swarm_ir::{{module::{{Module, WasmModuleConf}}, irparse::IRParser, pipeline_compile_module_to_wasm}};

fn get_function_bytes(full_wasm: &[u8]) -> &[u8] {{
    for r in wasmparser::Parser::new(0).parse_all(full_wasm) {{
        if let wasmparser::Payload::CodeSectionEntry(body) = r.unwrap() {{
            return body.range().slice(full_wasm);
        }}
    }}
    panic!("No function body found")
}}

    "#)
}

fn generate_test_function(w: &mut impl Write, test_name: &str, ir_input: &str, wat_input: &str) -> std::io::Result<()> {
    write!(w, r#"
#[test]
pub fn {}() {{
    let mut m = Module::default();
    let f = {{
        let mut p = IRParser::new(&mut m, "{}");
        p.parse_function().unwrap()
    }};
    m.add_function(f);
    let wasm_bytes_ir = pipeline_compile_module_to_wasm(m, false);

    assert!(wasmparser::validate(&wasm_bytes_ir).is_ok(), "Invalid WASM produced by IR compilation");

    let wasm_bytes_wat = wat::parse_str("{}").unwrap();

    let error_message = format!("Produced WASM isn't equal.
IR WebAssembly:
{{}}

WAT WebAssembly:
{{}}
    ", wasmprinter::print_bytes(&wasm_bytes_ir).unwrap(), wasmprinter::print_bytes(&wasm_bytes_wat).unwrap());

    assert_eq!(get_function_bytes(&wasm_bytes_ir), get_function_bytes(&wasm_bytes_wat), "{{}}", error_message);
}}
    "#, test_name, ir_input.replace('"', "\\\""), wat_input.replace('"', "\\\""))
}