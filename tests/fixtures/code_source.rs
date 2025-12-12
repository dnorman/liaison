// liaison id=example
fn main() {
    let x = 5 < 10;
    println!("Hello & goodbye");
}
// liaison end

// liaison id=generic-code
fn process<T>(value: T) -> Option<T> {
    if value < threshold { Some(value) } else { None }
}
// liaison end
