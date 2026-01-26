pub fn snake_to_pascal(snake: &str) -> String {
    let mut text = Vec::from(snake.as_bytes());
    let modified = text
        .as_mut_slice()
        .split_mut(|b| *b == b'_')
        .filter(|word| !word.is_empty())
        .flat_map(|word| {
            word[0] = word[0].to_ascii_uppercase();
            word as &[u8]
        })
        .copied()
        .collect::<Vec<_>>();
    unsafe { String::from_utf8_unchecked(modified) }
}

#[test]
fn boo() {
    assert_eq!(snake_to_pascal("fasdf_fae_Ege_4"), "FasdfFaeEge4");
    assert_eq!(snake_to_pascal("fa__sdf_fae_Ege_4"), "FaSdfFaeEge4");
    assert_eq!(snake_to_pascal("_fasdf_fae_Ege_4_"), "FasdfFaeEge4");
}
