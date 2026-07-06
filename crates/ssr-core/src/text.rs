//! Traduction des touches clavier en texte saisi (disposition US, au mieux).

pub use device_query::Keycode;

/// Jeton produit par une touche.
pub enum KeyToken {
    /// Texte à ajouter au tampon.
    Insert(String),
    /// Effacement du dernier caractère.
    Backspace,
}

/// Convertit une touche en jeton de texte selon l'état de la touche Maj.
///
/// Renvoie `None` pour les touches sans représentation textuelle (flèches,
/// touches de fonction, modificateurs, etc.).
pub fn key_to_token(key: &Keycode, shift: bool) -> Option<KeyToken> {
    use Keycode::*;

    // Lettres : casse selon Maj.
    if let Some(c) = letter(key) {
        let s = if shift { c.to_ascii_uppercase() } else { c };
        return Some(KeyToken::Insert(s.to_string()));
    }

    let text = match (key, shift) {
        (Key1, false) => "1",
        (Key1, true) => "!",
        (Key2, false) => "2",
        (Key2, true) => "@",
        (Key3, false) => "3",
        (Key3, true) => "#",
        (Key4, false) => "4",
        (Key4, true) => "$",
        (Key5, false) => "5",
        (Key5, true) => "%",
        (Key6, false) => "6",
        (Key6, true) => "^",
        (Key7, false) => "7",
        (Key7, true) => "&",
        (Key8, false) => "8",
        (Key8, true) => "*",
        (Key9, false) => "9",
        (Key9, true) => "(",
        (Key0, false) => "0",
        (Key0, true) => ")",
        (Numpad0, _) => "0",
        (Numpad1, _) => "1",
        (Numpad2, _) => "2",
        (Numpad3, _) => "3",
        (Numpad4, _) => "4",
        (Numpad5, _) => "5",
        (Numpad6, _) => "6",
        (Numpad7, _) => "7",
        (Numpad8, _) => "8",
        (Numpad9, _) => "9",
        (NumpadAdd, _) => "+",
        (NumpadSubtract, _) => "-",
        (NumpadMultiply, _) => "*",
        (NumpadDivide, _) => "/",
        (NumpadDecimal, _) => ".",
        (Space, _) => " ",
        (Enter | NumpadEnter, _) => "\n",
        (Tab, _) => "\t",
        (Minus, false) => "-",
        (Minus, true) => "_",
        (Equal, false) => "=",
        (Equal, true) => "+",
        (LeftBracket, false) => "[",
        (LeftBracket, true) => "{",
        (RightBracket, false) => "]",
        (RightBracket, true) => "}",
        (BackSlash, false) => "\\",
        (BackSlash, true) => "|",
        (Semicolon, false) => ";",
        (Semicolon, true) => ":",
        (Apostrophe, false) => "'",
        (Apostrophe, true) => "\"",
        (Comma, false) => ",",
        (Comma, true) => "<",
        (Dot, false) => ".",
        (Dot, true) => ">",
        (Slash, false) => "/",
        (Slash, true) => "?",
        (Grave, false) => "`",
        (Grave, true) => "~",
        (Backspace, _) => return Some(KeyToken::Backspace),
        _ => return None,
    };
    Some(KeyToken::Insert(text.to_string()))
}

/// Renvoie `true` si la touche est une touche Maj (gauche ou droite).
pub fn is_shift(key: &Keycode) -> bool {
    matches!(key, Keycode::LShift | Keycode::RShift)
}

fn letter(key: &Keycode) -> Option<char> {
    use Keycode::*;
    Some(match key {
        A => 'a',
        B => 'b',
        C => 'c',
        D => 'd',
        E => 'e',
        F => 'f',
        G => 'g',
        H => 'h',
        I => 'i',
        J => 'j',
        K => 'k',
        L => 'l',
        M => 'm',
        N => 'n',
        O => 'o',
        P => 'p',
        Q => 'q',
        R => 'r',
        S => 's',
        T => 't',
        U => 'u',
        V => 'v',
        W => 'w',
        X => 'x',
        Y => 'y',
        Z => 'z',
        _ => return None,
    })
}
