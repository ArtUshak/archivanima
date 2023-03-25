use html_escape::{encode_quoted_attribute, encode_text};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Breadcrumb {
    pub page_name: String,
    pub page_url: Option<String>,
}

impl Breadcrumb {
    pub fn new_with_url(page_name: String, page_url: String) -> Self {
        Self {
            page_name,
            page_url: Some(page_url),
        }
    }

    pub fn new_without_url(page_name: String) -> Self {
        Self {
            page_name,
            page_url: None,
        }
    }

    pub fn render(&self) -> String {
        match &self.page_url {
            Some(page_url_real) => {
                "<a href=\"".to_string()
                    + &encode_quoted_attribute(&page_url_real)
                    + "\">"
                    + &encode_text(&self.page_name)
                    + "</a>"
            }
            None => encode_text(&self.page_name).to_string(),
        }
    }
}
