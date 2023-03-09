use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::json;
use serde_yaml;
use std::fs::File;
use std::io::{BufReader, Read};

// Define the YAML dialog object
#[derive(Deserialize)]
struct Dialog {
    id: String,
    text: String,
    options: Vec<Option>,
}

#[derive(Deserialize)]
struct Option {
    option: String,
    next_id: String,
}

#[derive(serde::Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(serde::Deserialize)]
struct OpenAIChoice {
    text: String,
}

// Define the YAML prompt_decision template object
struct PromptDecisionTemplate(String);

impl PromptDecisionTemplate {
    fn new(file_path: &str) -> Self {
        let mut file = File::open(file_path).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        Self(contents)
    }

    fn format(&self, decision_prompt: &str, option_list: &str, user_response: &str) -> String {
        self.0
            .replace("{decision_prompt}", decision_prompt)
            .replace("{option_list}", option_list)
            .replace("{user_response}", user_response)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load the YAML file
    let file = File::open("dialog.yaml")?;
    let reader = BufReader::new(file);
    let dialog: Vec<Dialog> = serde_yaml::from_reader(reader)?;

    // Load the YAML prompt_decision template file
    let prompt_decision_template = PromptDecisionTemplate::new("prompt_decision_template.yaml");

    let agent = "Sortium";

    // Initialize the dialog
    let mut current_id = "start".to_string();

    loop {
        // Find the current dialog object
        let current_dialog = dialog
            .iter()
            .find(|obj| obj.id == current_id)
            .ok_or("Oops, something went wrong. Please try again.")?;

        // Print the current text and options
        println!("{}: {}", agent, current_dialog.text);
        for option in &current_dialog.options {
            println!("- {}", option.option);
        }

        // Map options to options.option
        let options: Vec<&String> = current_dialog.options.iter().map(|o| &o.option).collect();

        // Prompt the user for input
        let mut user_input = String::new();
        std::io::stdin().read_line(&mut user_input)?;

        // Replace the placeholders in the prompt_decision template with actual text
        let decision_prompt = current_dialog.text.clone();
        let option_list = serde_yaml::to_string(&options)?;
        let user_response = user_input.trim();

        // Create the prompt_decision
        let prompt_decision =
            prompt_decision_template.format(&decision_prompt, &option_list, &user_response);

        // Send the request to OpenAI
        let client = reqwest::blocking::Client::new();
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!(
                "Bearer {}",
                std::env::var("OPENAI_API_KEY").unwrap()
            ))?,
        );
        let response = client
            .post("https://api.openai.com/v1/completions")
            .headers(headers)
            .json(&json!({
                "model": "text-davinci-003",
                "prompt": prompt_decision,
                "suffix": "\n\n",
                "temperature": 0.7,
                "max_tokens": 256,
                "top_p": 1,
                "frequency_penalty": 0,
                "presence_penalty": 0
            }))
            .send()?;

        // Parse the response from OpenAI
        let response_text = response.text()?;
        let response_json: OpenAIResponse = serde_json::from_str(&response_text)?;
        let choice = response_json
            .choices
            .get(0)
            .ok_or("OpenAI did not return any choices")?
            .text
            .trim()
            .to_string();

        // Try to match the user's response with one of the options
        let choice_index = current_dialog
            .options
            .iter()
            .position(|o| o.option == choice);

        match choice_index {
            Some(index) => {
                // Get the next dialog ID based on the user's choice
                let next_id = current_dialog.options[index].next_id.clone();

                if next_id == "exit" {
                    // If the user chooses to exit, end the dialog
                    println!("{}: Thank you for using the dialog system.", agent);
                    break;
                } else {
                    // Otherwise, continue to the next dialog
                    current_id = next_id;
                }
            }
            None => {
                // If no match is found, repeat the prompt
                println!("{}: I'm sorry, I didn't understand your response.", agent);
            }
        }
    }

    Ok(())
}
