#![forbid(unsafe_code)]

use dns::idn_punycode::{IdnaConverter, IdnaError};

#[test]
fn test_to_ascii_valid() {
    // Normal ASCII
    assert_eq!(IdnaConverter::to_ascii("example.com").unwrap(), "example.com");

    // Punycode conversion
    assert_eq!(IdnaConverter::to_ascii("münchen.de").unwrap(), "xn--mnchen-3ya.de");

    // Check root dot
    assert_eq!(IdnaConverter::to_ascii("münchen.de.").unwrap(), "xn--mnchen-3ya.de.");
}

#[test]
fn test_to_unicode_valid() {
    assert_eq!(IdnaConverter::to_unicode("xn--mnchen-3ya.de"), "münchen.de");
    assert_eq!(IdnaConverter::to_unicode("example.com"), "example.com");
}

#[test]
fn test_homoglyph_detection() {
    // Mixed Latin and Cyrillic (Cyrillic 'о' U+043E)
    let mixed = "micrоsoft.com"; // 2nd o is cyrillic
    assert_eq!(IdnaConverter::to_ascii(mixed), Err(IdnaError::HomoglyphDetected));

    // Mixed Latin and Greek (Greek 'ο' U+03BF)
    let mixed_greek = "gοogle.com"; // 1st o is greek
    assert_eq!(IdnaConverter::to_ascii(mixed_greek), Err(IdnaError::HomoglyphDetected));

    // Pure Cyrillic is allowed, no homoglyph error (but might fail if other rules break, though valid IDNA should pass)
    let pure_cyrillic = "россия.рф";
    assert!(IdnaConverter::to_ascii(pure_cyrillic).is_ok());

    // Pure Latin is allowed
    let pure_latin = "microsoft.com";
    assert!(IdnaConverter::to_ascii(pure_latin).is_ok());
}

#[test]
fn test_label_too_long() {
    let long_label = "a".repeat(64) + ".com";
    assert_eq!(IdnaConverter::to_ascii(&long_label), Err(IdnaError::LabelTooLong));

    let valid_long_label = "a".repeat(63) + ".com";
    assert!(IdnaConverter::to_ascii(&valid_long_label).is_ok());
}

#[test]
fn test_domain_too_long() {
    // 253 characters is max length
    // Need to create a domain of 254 chars with labels <= 63.
    // 63 + 1 + 63 + 1 + 63 + 1 + 63 = 255 chars
    let label = "a".repeat(63);
    let domain = format!("{}.{}.{}.{}", label, label, label, label);
    assert!(domain.len() > 253);
    assert_eq!(IdnaConverter::to_ascii(&domain), Err(IdnaError::DomainTooLong));
}

#[test]
fn test_hyphen_placement() {
    // Starts with hyphen
    assert_eq!(IdnaConverter::to_ascii("-example.com"), Err(IdnaError::InvalidHyphenPlacement));

    // Ends with hyphen
    assert_eq!(IdnaConverter::to_ascii("example-.com"), Err(IdnaError::InvalidHyphenPlacement));

    // Double hyphen at pos 3 and 4 not xn--
    assert_eq!(IdnaConverter::to_ascii("ex--ample.com"), Err(IdnaError::InvalidHyphenPlacement));

    // xn-- should pass
    assert!(IdnaConverter::to_ascii("xn--mnchen-3ya.de").is_ok());

    // Valid hyphens
    assert!(IdnaConverter::to_ascii("e-xample.com").is_ok());
    assert!(IdnaConverter::to_ascii("ex-ample.com").is_ok());
}
