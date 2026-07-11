use super::payload::ProblemPayload;
use crate::config::settings::Configuration;
use crate::core::scaffold::Scaffold;
use crate::utils::ui::Ui;
use anyhow::{Context, Result};
use colored::Colorize;
use tiny_http::{Method, Response, Server};

pub struct ProblemListener;

impl ProblemListener {
    pub fn start(port: u16, use_short: bool, config: &Configuration) -> Result<()> {
        let server = Server::http(format!("127.0.0.1:{}", port)).map_err(|e| {
            anyhow::anyhow!("Failed to start HTTP listener on port {}: {}", port, e)
        })?;

        Ui::info(format!(
            "Listening for Competitive Companion on port {}...",
            port
        ));
        Ui::info("Click the green plus icon in your browser to send a problem.");

        for request in server.incoming_requests() {
            if let Err(e) = Self::handle_request(request, use_short, config) {
                Ui::warn(format!("Parser request error: {:#}", e));
            }
        }
        Ok(())
    }

    fn handle_request(
        mut request: tiny_http::Request,
        use_short: bool,
        config: &Configuration,
    ) -> Result<()> {
        if request.method() != &Method::Post {
            let method_str = format!("{:?}", request.method());

            let response = Response::from_string("Method Not Allowed").with_status_code(405);
            let _ = request.respond(response);

            anyhow::bail!("Unsupported HTTP method: {}", method_str);
        }

        const MAX_PAYLOAD_SIZE: usize = 10 * 1024 * 1024;
        let content_length = request.body_length().unwrap_or(0);
        if content_length > MAX_PAYLOAD_SIZE {
            let response = Response::from_string("Payload Too Large").with_status_code(413);
            let _ = request.respond(response);

            anyhow::bail!(
                "Payload size ({} bytes) exceeds safety limit",
                content_length
            );
        }

        match Self::parse_and_process(&mut request, use_short, config) {
            Ok(_) => {
                let response = Response::from_string("OK").with_status_code(200);
                let _ = request.respond(response);
                Ok(())
            }
            Err(e) => {
                let response =
                    Response::from_string(format!("Bad Request: {:#}", e)).with_status_code(400);
                let _ = request.respond(response);
                Err(e)
            }
        }
    }

    fn parse_and_process(
        request: &mut tiny_http::Request,
        use_short: bool,
        config: &Configuration,
    ) -> Result<()> {
        let mut body = Vec::new();
        request
            .as_reader()
            .read_to_end(&mut body)
            .context("Failed to read HTTP request body stream")?;

        let payload: ProblemPayload = serde_json::from_slice(&body)
            .context("Failed to deserialize JSON body into ProblemPayload")?;

        Self::process_problem(payload, use_short, config)?;
        Ok(())
    }

    fn process_problem(
        payload: ProblemPayload,
        use_short: bool,
        config: &Configuration,
    ) -> Result<()> {
        let mut target_name = payload.name.clone();
        let mut should_lowercase = true;

        if use_short {
            let short_name_opt = Self::generate_short_name(&payload.url);
            if let Some(short) = short_name_opt {
                target_name = short;
                should_lowercase = false;
            }
        }

        let sanitized_name = Self::sanitize_name(&target_name, should_lowercase);

        Ui::section("Problem Received");
        if target_name != payload.name {
            Ui::meta("name", format!("{} ({})", target_name, payload.name));
        } else {
            Ui::meta("name", &payload.name);
        }
        Ui::meta("group", &payload.group);
        Ui::meta(
            "limits",
            format!("{} ms / {} MB", payload.time_limit, payload.memory_limit),
        );

        Scaffold::from_parsed(&sanitized_name, &payload, config)?;

        crate::core::tester::Tester::save_tests(&sanitized_name, &payload.tests)?;

        Ui::ok(format!("Saved {} test cases", payload.tests.len()));
        println!(
            "\n  {} Run {} to test against the cases.",
            "󰄘".green(),
            format!("argo test {}", sanitized_name).cyan().bold()
        );

        Ok(())
    }

    fn sanitize_name(name: &str, lowercase: bool) -> String {
        let mapped_chars = name
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' });

        let mapped_string: String = mapped_chars.collect();
        let normalized = mapped_string.replace("__", "_");
        let trimmed = normalized.trim_matches('_');

        if lowercase {
            trimmed.to_lowercase()
        } else {
            trimmed.to_string()
        }
    }

    fn generate_short_name(url: &str) -> Option<String> {
        let url_lower = url.to_lowercase();

        let split_url = url.split('/');
        let filtered_parts = split_url.filter(|s| !s.is_empty());
        let parts: Vec<&str> = filtered_parts.collect();

        let get_after = |keyword: &str, offset: usize| {
            let position = parts.iter().position(|&p| p == keyword);
            position.and_then(|i| parts.get(i + offset).copied())
        };

        if url_lower.contains("codeforces.") {
            let contest_id = get_after("contest", 1);
            let problem_id = get_after("problem", 1);

            if let Some(contest) = contest_id
                && let Some(problem) = problem_id {
                    let combined = format!("{}{}", contest, problem);
                    return Some(combined.to_uppercase());
                }

            if parts.contains(&"problemset") {
                let problem_group = get_after("problem", 1);
                let problem_letter = get_after("problem", 2);

                if let Some(group) = problem_group
                    && let Some(letter) = problem_letter {
                        let combined = format!("{}{}", group, letter);
                        return Some(combined.to_uppercase());
                    }
            }
        }

        let simple_domains = [
            ("atcoder.jp", "tasks"),
            ("cses.fi", "task"),
            ("kattis.com", "problems"),
        ];

        for (domain, keyword) in simple_domains {
            if url_lower.contains(domain) {
                let target_part = get_after(keyword, 1);
                return target_part.map(|s| s.to_lowercase());
            }
        }

        None
    }
}
