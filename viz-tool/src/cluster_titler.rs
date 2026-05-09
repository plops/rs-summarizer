use crate::data_loader::EmbeddingPoint;
use crate::errors::VizError;
use gemini_rust::{Gemini, Model};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Extracts the abstract block from a summary.
/// Searches for "**Abstract**:" (case-insensitive) and returns the text
/// until the first timestamp marker (\n.*\d+:\d{2}).
/// Returns None if no Abstract marker is found.
pub fn extract_abstract_block(summary: &str) -> Option<String> {
    // Look for "**Abstract**:" (case-insensitive)
    let lines: Vec<&str> = summary.lines().collect();
    
    for (i, line) in lines.iter().enumerate() {
        if line.to_lowercase().contains("**abstract**:") {
            // Find the position after the marker
            let marker_pos = line.find("**abstract**:")? + "**abstract**:".len();
            let mut abstract_text = String::new();
            
            // Add text from the current line after the marker
            if marker_pos < line.len() {
                abstract_text.push_str(&line[marker_pos..].trim());
                abstract_text.push(' ');
            }
            
            // Add subsequent lines until we hit a timestamp marker
            for j in (i + 1)..lines.len() {
                let current_line = lines[j];
                // Check for timestamp pattern (digit:digit)
                if current_line.chars().next().map_or(false, |c| c == '*') && 
                   current_line.chars().any(|c| c.is_ascii_digit()) && 
                   current_line.contains(':') {
                    break;
                }
                abstract_text.push_str(current_line);
                abstract_text.push(' ');
            }
            
            return Some(abstract_text.trim().to_string());
        }
    }
    
    None
}

/// Generates cluster titles using the Gemini API.
/// Batches prompts up to 20,000 words, then makes separate API calls.
/// Uses JSON schema for response format: [{"id": <cluster_id>, "title": "<titel>"}]
pub async fn generate_titles(
    points: &[EmbeddingPoint],
    labels: &[i32],
    api_key: &str,
    model_name: &str,
) -> Result<HashMap<i32, String>, VizError> {
    // Group points by cluster and extract abstracts
    let mut cluster_data: HashMap<i32, Vec<String>> = HashMap::new();
    
    for (point, &label) in points.iter().zip(labels.iter()) {
        if let Some(abstract_block) = extract_abstract_block(&point.summary) {
            cluster_data.entry(label).or_insert_with(Vec::new).push(abstract_block);
        }
    }
    
    // Create prompts for each cluster
    let mut cluster_prompts: Vec<(i32, String)> = Vec::new();
    for (&cluster_id, abstracts) in &cluster_data {
        let prompt = create_cluster_prompt(cluster_id, abstracts);
        cluster_prompts.push((cluster_id, prompt));
    }
    
    // Process in batches up to 20,000 words
    let mut all_titles = HashMap::new();
    let mut current_batch = Vec::new();
    let mut current_word_count = 0usize;
    const WORD_LIMIT: usize = 20000;
    
    for (cluster_id, prompt) in cluster_prompts {
        let prompt_words = prompt.split_whitespace().count();
        
        if current_word_count + prompt_words > WORD_LIMIT && !current_batch.is_empty() {
            // Process current batch
            let batch_titles = process_batch(&current_batch, api_key, model_name).await?;
            all_titles.extend(batch_titles);
            
            // Start new batch
            current_batch.clear();
            current_word_count = 0;
        }
        
        current_batch.push((cluster_id, prompt));
        current_word_count += prompt_words;
    }
    
    // Process final batch
    if !current_batch.is_empty() {
        let batch_titles = process_batch(&current_batch, api_key, model_name).await?;
        all_titles.extend(batch_titles);
    }
    
    Ok(all_titles)
}

fn create_cluster_prompt(cluster_id: i32, abstracts: &[String]) -> String {
    let abstracts_text = abstracts.iter()
        .take(10) // Limit to 10 abstracts to keep prompt manageable
        .enumerate()
        .map(|(i, abstract_text)| format!("{}. {}\n", i + 1, abstract_text))
        .collect::<String>();
    
    format!(
        r#"You are analyzing a cluster of transcript summaries with cluster ID {}.

Here are sample abstracts from this cluster:

{}

Based on these abstracts, provide a concise, descriptive title for this cluster that captures the main theme or topic. The title should be:
- Descriptive and informative
- No more than 60 characters
- In English
- Focused on the content domain or subject matter

Please respond with a JSON array containing the cluster ID and title:
[{{"id": {}, "title": "Your title here"}}]"#,
        cluster_id, abstracts_text, cluster_id
    )
}

async fn process_batch(
    batch: &[(i32, String)],
    api_key: &str,
    model_name: &str,
) -> Result<HashMap<i32, String>, VizError> {
    // Combine all prompts in the batch
    let combined_prompt = batch.iter()
        .map(|(_, prompt)| format!("{}\n\n---\n\n", prompt))
        .collect::<String>();
    
    // Create Gemini client
    let client = Gemini::with_model(api_key, Model::Custom(format!("models/{}", model_name)))
        .map_err(|e| VizError::Api(format!("Failed to create Gemini client: {}", e)))?;
    
    let response = client
        .generate_content()
        .with_user_message(&combined_prompt)
        .with_response_mime_type("application/json")
        .execute()
        .await
        .map_err(|e| VizError::Api(format!("Gemini API call failed: {}", e)))?;
    
    let response_text = response.text();
    
    // Parse JSON response
    let parsed_response: Vec<ClusterTitleResponse> = serde_json::from_str(&response_text)
        .map_err(|e| VizError::Api(format!("Failed to parse Gemini response as JSON: {}", e)))?;
    
    // Convert to HashMap
    let mut titles = HashMap::new();
    for item in parsed_response {
        titles.insert(item.id, item.title);
    }
    
    Ok(titles)
}

#[derive(Debug, Serialize, Deserialize)]
struct ClusterTitleResponse {
    id: i32,
    title: String,
}

/// Saves cluster titles to a JSON file.
pub fn save_titles(titles: &HashMap<i32, String>, path: &Path) -> Result<(), VizError> {
    let json = serde_json::to_string_pretty(titles)
        .map_err(|e| VizError::SerializationError(format!("Failed to serialize cluster titles: {}", e)))?;
    
    fs::write(path, json)
        .map_err(|e| VizError::Io(e))?;
    
    Ok(())
}

/// Loads cluster titles from a JSON file.
pub fn load_titles(path: &Path) -> Result<HashMap<i32, String>, VizError> {
    let json = fs::read_to_string(path)
        .map_err(|e| VizError::Io(e))?;
    
    let titles: HashMap<i32, String> = serde_json::from_str(&json)
        .map_err(|e| VizError::SerializationError(format!("Failed to parse cluster titles: {}", e)))?;
    
    Ok(titles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: embedding-visualization, Property 6: Abstract Block Extraction
    proptest! {
        #[test]
        fn prop_abstract_extraction(
            abstract_text in "[a-zA-Z0-9 .,!?]{1,200}",
            suffix in "[a-zA-Z0-9 .,!?]{0,50}"
        ) {
            let timestamp = "\n* 0:00 Some section";
            let summary = format!("**Abstract**:\n\n{}{}{}", abstract_text, suffix, timestamp);
            let result = extract_abstract_block(&summary);
            prop_assert!(result.is_some());
            let block = result.unwrap();
            // Block should contain the abstract text
            prop_assert!(block.contains(&abstract_text[..abstract_text.len().min(50)]));
            // Block should not contain the timestamp marker
            prop_assert!(!block.contains("0:00"));
        }
    }

    #[test]
    fn test_extract_abstract_no_marker() {
        let summary = "This is a summary without an abstract marker.\nJust some regular text.";
        let result = extract_abstract_block(summary);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_abstract_no_timestamp() {
        let summary = "**Abstract**:\n\nThis is the abstract content.\nNo timestamp here.";
        let result = extract_abstract_block(summary);
        assert!(result.is_some());
        let block = result.unwrap();
        assert!(block.contains("This is the abstract content."));
        assert!(block.contains("No timestamp here."));
    }

    #[test]
    fn test_extract_abstract_case_insensitive() {
        let summary = "**abstract**:\n\nThis should work with lowercase.\n* 1:23 timestamp";
        let result = extract_abstract_block(summary);
        assert!(result.is_some());
        let block = result.unwrap();
        assert!(block.contains("This should work with lowercase."));
        assert!(!block.contains("1:23"));
    }
}
