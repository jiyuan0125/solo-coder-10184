use jaq_core::load::{Arena, File, Loader};
use jaq_json::Val;
use serde_json::{from_value, json};

fn main() {
    let x: Val = from_value(json!({"a": 1})).unwrap();
    let code = "(try .a catch 0) |= . + 1";
    
    let arena = Arena::default();
    let loader = Loader::new(jaq_core::defs());
    let modules = loader.load(&arena, File { path: (), code }).unwrap();
    let filter = jaq_core::Compiler::default()
        .with_funs(jaq_core::funs())
        .compile(modules)
        .unwrap();
    
    let ctx = jaq_core::filter::Ctx::<jaq_core::data::JustLut<Val>>::new(
        &filter.lut, 
        jaq_core::filter::Vars::new([])
    );
    let out: Vec<_> = filter.id.run((ctx, x))
        .map(jaq_core::unwrap_valr)
        .collect();
    
    println!("Output: {:?}", out);
    println!("Expected: Ok(Val::from(json!(2)))");
    
    // Also test the second case
    let x2: Val = from_value(json!({})).unwrap();
    let ctx2 = jaq_core::filter::Ctx::<jaq_core::data::JustLut<Val>>::new(
        &filter.lut, 
        jaq_core::filter::Vars::new([])
    );
    let out2: Vec<_> = filter.id.run((ctx2, x2))
        .map(jaq_core::unwrap_valr)
        .collect();
    println!("\nInput {{}}: {:?}", out2);
}
