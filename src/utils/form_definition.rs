use std::borrow::Cow;

use chrono::NaiveDateTime;
use html_escape::{encode_quoted_attribute, encode_text};
use serde::{Deserialize, Serialize};
use validator::{ValidationError, ValidationErrors};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum FieldData {
    Checkbox(bool),
    Radio(Vec<(String, String)>, Option<String>),
    #[serde(deserialize_with = "crate::utils::form_serialization::deserialize_option_from_str")]
    #[serde(serialize_with = "crate::utils::form_serialization::serialize_option_debug")]
    Date(Option<NaiveDateTime>),
    Number(Option<f64>),
    EMail(Option<String>),
    Hidden(Option<String>),
    Telephone(Option<String>),
    Text(Option<String>),
    TextArea(Option<String>),
    Password(Option<String>),
    Url(Option<String>),
}

#[derive(Clone, Debug)]
pub struct FieldDefinition {
    pub name: String,
    pub verbose_name: String,
    pub field_type: FieldData,
    pub errors: Vec<ValidationError>,
}

impl FieldDefinition {
    fn render_errors(&self) -> String {
        if self.errors.is_empty() {
            "".to_string()
        } else {
            let mut result = "<ul>".to_string();
            for error in self.errors.iter() {
                let error_message = error.message.clone();
                result += &("<li>".to_string()
                    + &encode_text(
                        error_message
                            .unwrap_or_else(|| Cow::Owned("Error".to_string()))
                            .as_ref(),
                    )
                    + "</li>");
            }
            result += "</ul>";
            result
        }
    }

    pub fn render(&self) -> String {
        // TODO: escape values

        let name_escaped = &encode_quoted_attribute(&self.name);
        let verbose_name_escaped = &encode_text(&self.verbose_name);
        match self.field_type.clone() {
            FieldData::Checkbox(true) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<input type=\"checkbox\" id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" checked /></td></tr>\n"
            }
            FieldData::Checkbox(false) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<input type=\"checkbox\" id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" /></td></tr>\n"
            }
            FieldData::Radio(options, selected_option) => {
                let options_str: String = options
                    .iter()
                    .map(|(option_name, option_verbose_name)| {
                        let option_name_escaped = encode_quoted_attribute(option_name);
                        "<div><input type=\"radio\" id=\"input-".to_string()
                            + name_escaped
                            + "-"
                            + &option_name_escaped
                            + "\""
                            + {
                                if selected_option.as_ref() == Some(option_name) {
                                    " checked"
                                } else {
                                    ""
                                }
                            }
                            + " name=\""
                            + name_escaped
                            + "\" value=\""
                            + &option_name_escaped
                            + "\" /><label for=\"input-"
                            + name_escaped
                            + "-"
                            + &option_name_escaped
                            + "\">"
                            + &encode_text(option_verbose_name)
                            + "</label></div>"
                    })
                    .collect();

                "<tr><th>".to_string()
                    + verbose_name_escaped
                    + "</th><td><fieldset>"
                    + &self.render_errors()
                    + &options_str
                    + "</fieldset></td></tr>\n"
            }
            FieldData::Number(Some(number)) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<input type=\"number\" id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" value=\""
                    + &number.to_string()
                    + "\" /></td></tr>\n"
            }
            FieldData::Number(None) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<input type=\"number\" id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" /></td></tr>\n"
            }
            FieldData::Date(Some(date)) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<input type=\"date\" id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" value=\""
                    + &date.to_string()
                    + "\" /></td></tr>\n"
            }
            FieldData::Date(None) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<input type=\"date\" id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" /></td></tr>\n"
            }
            FieldData::Hidden(None) => {
                "<tr style=\"display: none\"><td></td><td><input type=\"hidden\" id=\"input-"
                    .to_string()
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" /></td></tr>\n"
            }
            FieldData::Hidden(Some(hidden)) => {
                "<tr style=\"display: none\"><td></td><td><input type=\"hidden\" id=\"input-"
                    .to_string()
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" value=\""
                    + &encode_text(&hidden)
                    + "\" /></td></tr>\n"
            }
            FieldData::EMail(Some(email)) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<input type=\"email\" id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" value=\""
                    + &encode_text(&email)
                    + "\" /></td></tr>\n"
            }
            FieldData::EMail(None) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<input type=\"email\" id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" /></td></tr>\n"
            }
            FieldData::Telephone(Some(telephone)) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<input type=\"tel\" id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" value=\""
                    + &encode_text(&telephone)
                    + "\" /></td></tr>\n"
            }
            FieldData::Telephone(None) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<input type=\"tel\" id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" /></td></tr>\n"
            }
            FieldData::Text(Some(text)) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<input type=\"text\" id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" value=\""
                    + &encode_text(&text)
                    + "\" /></td></tr>\n"
            }
            FieldData::Text(None) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<input type=\"text\" id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" /></td></tr>\n"
            }
            FieldData::TextArea(Some(text)) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<textarea rows=10 id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\">"
                    + &encode_text(&text)
                    + "</textarea></td></tr>\n"
            }
            FieldData::TextArea(None) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<textarea rows=10 id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\"></textarea></td></tr>\n"
            }
            FieldData::Password(Some(password)) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<input type=\"password\" id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" value=\""
                    + &encode_text(&password)
                    + "\" /></td></tr>\n"
            }
            FieldData::Password(None) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<input type=\"password\" id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" /></td></tr>\n"
            }
            FieldData::Url(Some(url)) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<input type=\"url\" id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" value=\""
                    + &encode_text(&url)
                    + "\" /></td></tr>\n"
            }
            FieldData::Url(None) => {
                "<tr><th><label for=\"input-".to_string()
                    + name_escaped
                    + "\">"
                    + verbose_name_escaped
                    + "</label></th><td>"
                    + &self.render_errors()
                    + "<input type=\"url\" id=\"input-"
                    + name_escaped
                    + "\" name=\""
                    + name_escaped
                    + "\" /></td></tr>\n"
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct FormDefinition {
    pub fields: Vec<FieldDefinition>,
    pub submit_name: Option<String>,
}

impl FormDefinition {
    pub fn render(&self) -> String {
        let mut result =
            "<div class=\"table-wrapper\">\n<table class=\"table-detail\">\n".to_string();
        for field in self.fields.iter() {
            result += &field.render();
        }
        if let Some(submit_name) = self.submit_name.clone() {
            let submit_name_escaped = &encode_text(&submit_name);
            result += &("<tr><td></td><td><button>".to_string()
                + submit_name_escaped
                + "</button></td></tr>\n");
        }
        result += "</table>\n</div>";
        result
    }
}

pub trait FormWithDefinition {
    fn get_definition(&self, errors: ValidationErrors) -> FormDefinition;
}
