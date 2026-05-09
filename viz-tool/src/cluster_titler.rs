use crate::errors::VizError;
use gemini_rust::{Gemini, Model, Content};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterLabels {
    labels: HashMap<i32, String>,
}

impl Default for ClusterLabels {
    fn default() -> Self {
        Self {
            labels: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterInfo {
    pub cluster_id: i32,
    pub points_count: usize,
    pub center_4d: Vec<f32>,
    pub sample_points: Vec<Vec<String>>,
}

pub struct ClusterTitler {
    api_key: String,
    model_name: String,
}

impl ClusterTitler {
    pub fn new(api_key: String, model_name: String) -> Self {
        Self { api_key, model_name }
    }

    pub fn extract_abstract_block(summary: &str) -> Option<String> {
        // Look for "**Abstract:**" (case-insensitive)
        let lines: Vec<&str> = summary.lines().collect();
        
        for (i, line) in lines.iter().enumerate() {
            if line.to_lowercase().contains("**abstract:**") {
                // Extract everything from this line to the end
                if i + 1 < lines.len() {
                    let abstract_start = line.find("**abstract:**").unwrap() + "**abstract:**".len();
                    let abstract_text = &line[abstract_start..];
                    return Some(abstract_text.trim().to_string());
                }
            }
        }
        
        None
    }

    pub async fn generate_titles(
        &self, 
        points: &[[f32; 4]], 
        cluster_labels: &[i32], 
        api_key: &str,
        model_name: &str
    ) -> Result<ClusterLabels, VizError> {
        let mut cluster_info: HashMap<i32, ClusterInfo> = HashMap::new();
        
        // Group points by cluster
        for (i, &point) in points.iter().enumerate() {
            let cluster_id = cluster_labels[i];
            
            let entry = cluster_info.entry(cluster_id).or_insert_with(|| ClusterInfo {
                cluster_id,
                points_count: 0,
                center_4d: Vec::new(),
                sample_points: Vec::new(),
            });
            
            entry.points_count += 1;
            entry.center_4d.extend_from_slice(&point);
            
            // Store sample summaries (we'll need these for the abstract block)
            entry.sample_points.push(format!("Point {} summary placeholder", i));
        }

        // Calculate cluster centers
        for cluster in cluster_info.values_mut() {
            if cluster.points_count > 0 {
                let n = cluster.points_count as f64;
                let mut center = vec![0.0f32; 4];
                for coord in &cluster.center_4d {
                    center.push(*coord / n as f32);
                }
                cluster.center_4d = center;
            }
        }

        // Generate prompts for each cluster
        let mut prompts = Vec::new();
        for cluster in cluster_info.values() {
            let prompt = self.create_cluster_prompt(cluster);
            prompts.push(prompt);
        }

        // Create Gemini client
        let gemini = Gemini::with_model(&api_key, Model::Custom(format!("models/{}", model_name)))
            .map_err(|e| VizError::Api(format!("Failed to create Gemini client: {}", e)))?;

        // Process clusters in batches to avoid token limits
        let mut all_labels = HashMap::new();
        
        for (i, prompt) in prompts.iter().enumerate() {
            println!("Generating title for cluster {} of {}", i + 1, prompts.len());
            
            let content = gemini_rust::content::Content::new(prompt);
            
            let response = gemini
                .generate_content()
                .map_err(|e| VizError::Api(format!("Gemini API call failed: {}", e)))?;
            
            if response.candidates.is_empty() {
                return Err(VizError::Api("Empty response from Gemini API".to_string()));
            }

            let candidate = &response.candidates[0];
            let text = candidate.content.parts.iter()
                .find_map(|part| part.text.as_ref())
                .ok_or_else(|| VizError::Api("No text in Gemini response".to_string()))?;

            // Extract cluster ID from response (expect JSON format)
            let cluster_id = (i + 1) as i32;
            let title = text.trim().to_string();
            
            all_labels.insert(cluster_id, title);
            
            // Add delay to avoid rate limiting
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        Ok(ClusterLabels { labels: all_labels })
    }

    fn create_cluster_prompt(&self, cluster: &ClusterInfo) -> String {
        let center_str = cluster.center_4d.iter()
            .map(|coord| format!("{:.3}", coord))
            .collect::<Vec<_>>()
            .join(", ");

        let sample_count = std::cmp::min(5, cluster.points_count);
        
        format!(
            r#"You are analyzing clusters of 4D embeddings from a transcript summarization system.

Cluster {} Analysis:
- Points in cluster: {}
- 4D center: [{}]
- Sample points: {}

Please provide a concise, descriptive title for this cluster (max 50 characters) that captures the main theme or topic. Focus on the content patterns that would be found in the original transcripts.

Respond with only the title, no additional text."#,
            cluster.cluster_id,
            cluster.points_count,
            center_str,
            sample_count,
            cluster.sample_points.iter().take(sample_count).map(|v| v.to_string()).collect::<Vec<_>>().join("\n\n")
        )
    }

    pub fn save_titles(&self, labels: &ClusterLabels, path: &Path) -> Result<(), VizError> {
        let json = serde_json::to_string(labels)
            .map_err(|e| VizError::SerializationError(format!("Failed to serialize cluster labels: {}", e)))?;
        
        fs::write(path, json)
            .map_err(|e| VizError::Io(format!("Failed to write cluster labels: {}", e)))?;
        
        Ok(())
    }

    pub fn load_titles(path: &Path) -> Result<ClusterLabels, VizError> {
        let json = fs::read_to_string(path)
            .map_err(|e| VizError::Io(format!("Failed to read cluster labels: {}", e)))?;
        
        let labels: ClusterLabels = serde_json::from_str(&json)
            .map_err(|e| VizError::SerializationError(format!("Failed to parse cluster labels: {}", e)))?;
        
        Ok(labels)
    }
}
