use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct LocationData {
    pub provinces: Vec<Province>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Province {
    pub name: String,
    pub districts: Vec<District>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct District {
    pub name: String,
    pub sectors: Vec<Sector>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Sector {
    pub name: String,
    pub cells: Vec<Cell>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Cell {
    pub name: String,
    pub villages: Vec<Village>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Village {
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct LocationCounts {
    pub provinces: usize,
    pub districts: usize,
    pub sectors: usize,
    pub cells: usize,
    pub villages: usize,
}

#[derive(Debug, Clone)]
pub struct LocationRecord {
    pub id: String,
    pub province: String,
    pub district: String,
    pub sector: String,
    pub cell: String,
    pub village: String,
    pub breadcrumb: String,
    pub search_text: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LocationQuery {
    #[serde(default, alias = "q")]
    pub query: String,
    #[serde(default)]
    pub province: String,
    #[serde(default)]
    pub district: String,
    #[serde(default)]
    pub sector: String,
    #[serde(default)]
    pub cell: String,
    #[serde(default)]
    pub village: String,
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_per_page")]
    pub per_page: usize,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub delay_ms: u64,
}

fn default_page() -> usize {
    1
}

fn default_per_page() -> usize {
    20
}

impl LocationQuery {
    pub fn normalized(mut self) -> Self {
        self.page = self.page.max(1);
        self.per_page = self.per_page.clamp(1, 100);
        self.delay_ms = self.delay_ms.min(10_000);
        self
    }
}

#[derive(Debug, Clone)]
pub struct SearchPage {
    pub results: Vec<LocationRecord>,
    pub total: usize,
    pub page: usize,
    pub total_pages: usize,
    pub has_previous: bool,
    pub has_next: bool,
    pub previous_page: usize,
    pub next_page: usize,
}

#[derive(Debug, Clone)]
pub struct LocationDetails {
    pub location: LocationRecord,
    pub sibling_villages: Vec<String>,
}
