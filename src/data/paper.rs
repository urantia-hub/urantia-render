use anyhow::Result;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawNode {
    pub global_id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub paper_id: Option<String>,
    pub paper_title: Option<String>,
    pub part_id: Option<String>,
    pub section_id: Option<String>,
    pub section_title: Option<String>,
    pub paragraph_id: Option<String>,
    pub standard_reference_id: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Paragraph {
    pub global_id: String,
    pub standard_reference_id: String,
    pub text: String,
    pub section_title: Option<String>,
    pub section_id: String,
}

#[derive(Debug, Clone)]
pub struct Section {
    pub section_id: String,
    pub section_title: Option<String>,
    pub paragraphs: Vec<Paragraph>,
}

#[derive(Debug, Clone)]
pub struct Paper {
    pub paper_id: String,
    pub paper_title: String,
    pub part_id: String,
    pub sections: Vec<Section>,
}

impl Paper {
    pub fn from_json(json_str: &str) -> Result<Self> {
        let nodes: Vec<RawNode> = serde_json::from_str(json_str)?;

        let paper_entry = nodes
            .iter()
            .find(|n| n.node_type == "paper")
            .ok_or_else(|| anyhow::anyhow!("No paper entry found"))?;

        let paper_id = paper_entry.paper_id.clone().unwrap_or_default();
        let paper_title = paper_entry.paper_title.clone().unwrap_or_default();
        let part_id = paper_entry.part_id.clone().unwrap_or_default();

        let mut section_map: BTreeMap<String, Section> = BTreeMap::new();

        for node in &nodes {
            let section_id = node.section_id.clone().unwrap_or_else(|| "0".to_string());

            if node.node_type == "section" {
                section_map.entry(section_id.clone()).or_insert_with(|| Section {
                    section_id: section_id.clone(),
                    section_title: node.section_title.clone().filter(|s| !s.is_empty()),
                    paragraphs: Vec::new(),
                });
            }

            if node.node_type == "paragraph" {
                if let Some(text) = &node.text {
                    let section = section_map.entry(section_id.clone()).or_insert_with(|| Section {
                        section_id: section_id.clone(),
                        section_title: node.section_title.clone().filter(|s| !s.is_empty()),
                        paragraphs: Vec::new(),
                    });

                    section.paragraphs.push(Paragraph {
                        global_id: node.global_id.clone(),
                        standard_reference_id: node
                            .standard_reference_id
                            .clone()
                            .unwrap_or_default(),
                        text: text.clone(),
                        section_title: node.section_title.clone().filter(|s| !s.is_empty()),
                        section_id: section_id.clone(),
                    });
                }
            }
        }

        let mut sections: Vec<Section> = section_map.into_values().collect();
        sections.sort_by_key(|s| s.section_id.parse::<u32>().unwrap_or(0));

        Ok(Paper {
            paper_id,
            paper_title,
            part_id,
            sections,
        })
    }

    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let json_str = std::fs::read_to_string(path)?;
        Self::from_json(&json_str)
    }

    pub fn total_paragraphs(&self) -> usize {
        self.sections.iter().map(|s| s.paragraphs.len()).sum()
    }
}
