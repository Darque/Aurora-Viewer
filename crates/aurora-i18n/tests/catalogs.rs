use aurora_i18n::{SOURCES, resolve_language};
use fluent_syntax::ast;
use std::collections::{BTreeMap, BTreeSet};

fn inline(value: &ast::InlineExpression<&str>, variables: &mut BTreeSet<String>) {
    match value {
        ast::InlineExpression::VariableReference { id } => {
            variables.insert(id.name.to_owned());
        }
        ast::InlineExpression::FunctionReference { arguments, .. } => {
            for value in &arguments.positional {
                inline(value, variables);
            }
            for value in &arguments.named {
                inline(&value.value, variables);
            }
        }
        ast::InlineExpression::TermReference {
            arguments: Some(arguments),
            ..
        } => {
            for value in &arguments.positional {
                inline(value, variables);
            }
            for value in &arguments.named {
                inline(&value.value, variables);
            }
        }
        ast::InlineExpression::Placeable { expression: value } => expression(value, variables),
        _ => {}
    }
}

fn expression(value: &ast::Expression<&str>, variables: &mut BTreeSet<String>) {
    match value {
        ast::Expression::Inline(value) => inline(value, variables),
        ast::Expression::Select { selector, variants } => {
            inline(selector, variables);
            for variant in variants {
                pattern(&variant.value, variables);
            }
        }
    }
}

fn pattern(value: &ast::Pattern<&str>, variables: &mut BTreeSet<String>) {
    for element in &value.elements {
        if let ast::PatternElement::Placeable { expression: value } = element {
            expression(value, variables);
        }
    }
}

#[test]
fn only_english_is_bundled_and_the_catalog_is_valid() {
    assert_eq!(SOURCES.len(), 1);
    assert_eq!(SOURCES[0].0, "en");
    assert_eq!(aurora_i18n::available_languages().collect::<Vec<_>>(), [("en", "English")]);
    let (_, source) = SOURCES[0];
    let resource = fluent_syntax::parser::parse(source).expect("valid Fluent syntax");
    let mut messages = BTreeMap::new();
    for entry in resource.body {
        if let ast::Entry::Message(message) = entry {
            let mut variables = BTreeSet::new();
            if let Some(value) = &message.value {
                pattern(value, &mut variables);
            }
            for attribute in &message.attributes {
                pattern(&attribute.value, &mut variables);
            }
            assert!(
                messages.insert(message.id.name, variables).is_none(),
                "duplicate key {}",
                message.id.name
            );
        }
    }
    assert!(messages.contains_key("language-name"));
}

#[test]
fn unsupported_saved_and_system_languages_fall_back_to_english() {
    let languages = ["en"];
    assert_eq!(resolve_language("en", "fr-FR", &languages), "en");
    assert_eq!(resolve_language("default", "ja-JP", &languages), "en");
    assert_eq!(resolve_language("fr", "en-US", &languages), "en");
    assert_eq!(resolve_language("unknown", "de-DE", &languages), "en");
    assert_eq!(aurora_i18n::language(), "en");
}

#[test]
fn fluent_messages_format_with_their_declared_variables() {
    use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
    let (code, source) = SOURCES[0];
    let parsed = fluent_syntax::parser::parse(source).expect("Fluent syntax");
    let mut bundle = FluentBundle::new(vec![code.parse().expect("locale")]);
    bundle.set_use_isolating(false);
    bundle
        .add_resource(FluentResource::try_new(source.to_owned()).expect("catalog"))
        .expect("unique keys");
    for entry in parsed.body {
        let ast::Entry::Message(message) = entry else { continue };
        let mut variables = BTreeSet::new();
        pattern(message.value.as_ref().expect("message value"), &mut variables);
        let mut args = FluentArgs::new();
        for variable in variables {
            if variable == "count" {
                args.set(variable, 2);
            } else {
                args.set(variable, "Zoé");
            }
        }
        let value = bundle.get_message(message.id.name).expect("message").value().expect("pattern");
        let mut errors = Vec::new();
        let _ = bundle.format_pattern(value, Some(&args), &mut errors);
        assert!(errors.is_empty(), "{code}/{}: {errors:?}", message.id.name);
    }
}
