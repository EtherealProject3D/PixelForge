#[derive(Clone, Copy)]
pub(crate) enum Profile {
    General,
    Pony,
    NaturalLanguage,
}

pub(crate) fn suggest(input: &str, profile: Profile) -> String {
    let input = strip_request_prefix(input.trim());
    match profile {
        Profile::General => input.to_owned(),
        Profile::Pony => format_pony(input),
        Profile::NaturalLanguage => {
            // Leave weights, tags, multiline prompts and other structured syntax alone.
            if input.ends_with(char::is_alphanumeric)
                && input.contains(' ')
                && input
                    .chars()
                    .all(|c| c.is_alphanumeric() || " ,.'’-".contains(c))
            {
                format!("{input}.")
            } else {
                input.to_owned()
            }
        }
    }
}

fn strip_request_prefix(input: &str) -> &str {
    // Match only an opening request to create an image. Keep the entire
    // description, including nested requests, quoted text, and constraints.
    for prefix in [
        "generate an image of ",
        "create an image of ",
        "make an image of ",
        "please generate an image of ",
        "please create an image of ",
        "please make an image of ",
    ] {
        if let Some((opening, description)) = input.split_at_checked(prefix.len())
            && opening.eq_ignore_ascii_case(prefix)
            && !description.trim().is_empty()
        {
            return description.trim_start();
        }
    }
    input
}

fn format_pony(mut input: &str) -> String {
    let mut result = String::new();
    // Format only existing score tags at the start. Everything after them
    // may contain free prose or model syntax and is kept intact.
    while let Some((tag, rest)) = input.split_once(',') {
        let tag = tag.trim();
        if !matches!(
            tag,
            "score_9" | "score_8_up" | "score_7_up" | "score_6_up" | "score_5_up" | "score_4_up"
        ) {
            break;
        }
        result.push_str(tag);
        result.push(',');
        input = rest.trim_start();
        if !input.is_empty() {
            result.push(' ');
        }
    }
    result.push_str(input);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_only_an_opening_image_request() {
        let description = "two women in red coats, back view, seated, no hats";
        for profile in [Profile::General, Profile::Pony, Profile::NaturalLanguage] {
            assert_eq!(
                suggest(&format!("Please CREATE an image of {description}"), profile),
                suggest(description, profile)
            );
        }
        for prompt in [
            "A sign saying Create an image of a red fox",
            "Do not create an image of a red fox",
            "Create an image of",
            "éééééééééééé",
        ] {
            assert_eq!(suggest(prompt, Profile::General), prompt);
        }
    }

    #[test]
    fn preserves_front_and_back_view_details() {
        for description in [
            "Two women in red coats, front view, facing the camera, standing, no hats",
            "One man in a blue coat, back view, facing away, sitting, no mirrors",
        ] {
            assert_eq!(suggest(description, Profile::General), description);
            assert_eq!(suggest(description, Profile::Pony), description);
            assert_eq!(
                suggest(description, Profile::NaturalLanguage),
                format!("{description}.")
            );
        }
    }

    #[test]
    fn formats_existing_pony_scores_without_changing_weights_or_adding_tags() {
        let input = "score_9 ,score_7_up, (from behind:1.2), no score_8_up";
        let expected = "score_9, score_7_up, (from behind:1.2), no score_8_up";
        assert_eq!(suggest(input, Profile::Pony), expected);
        assert_eq!(suggest(expected, Profile::Pony), expected);
        assert_eq!(suggest(input, Profile::General), input);
        assert_eq!(suggest(input, Profile::NaturalLanguage), input);
    }

    #[test]
    fn leaves_structured_natural_language_inputs_intact() {
        for prompt in [
            "(front view:1.2), red coat",
            "front view\nback view",
            "from_behind",
        ] {
            assert_eq!(suggest(prompt, Profile::NaturalLanguage), prompt);
        }
        assert_eq!(suggest("  ", Profile::NaturalLanguage), "");
    }
}
