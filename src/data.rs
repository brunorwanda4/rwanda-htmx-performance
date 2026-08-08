use std::{collections::HashMap, fs, io, path::Path};

use crate::models::{
    LocationCounts, LocationData, LocationDetails, LocationQuery, LocationRecord, SearchPage,
};

pub struct LocationStore {
    hierarchy: LocationData,
    records: Vec<LocationRecord>,
    record_positions: HashMap<String, usize>,
    counts: LocationCounts,
}

impl LocationStore {
    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        let raw = fs::read_to_string(path)?;
        let hierarchy: LocationData = serde_json::from_str(&raw)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        let mut counts = LocationCounts {
            provinces: hierarchy.provinces.len(),
            ..LocationCounts::default()
        };
        let mut records = Vec::new();

        for province in &hierarchy.provinces {
            counts.districts += province.districts.len();
            for district in &province.districts {
                counts.sectors += district.sectors.len();
                for sector in &district.sectors {
                    counts.cells += sector.cells.len();
                    for cell in &sector.cells {
                        counts.villages += cell.villages.len();
                        for village in &cell.villages {
                            let breadcrumb = format!(
                                "{} / {} / {} / {} / {}",
                                province.name, district.name, sector.name, cell.name, village.name
                            );
                            let id = format!("v{:05}", records.len() + 1);
                            records.push(LocationRecord {
                                id,
                                province: province.name.clone(),
                                district: district.name.clone(),
                                sector: sector.name.clone(),
                                cell: cell.name.clone(),
                                village: village.name.clone(),
                                search_text: breadcrumb.to_lowercase(),
                                breadcrumb,
                            });
                        }
                    }
                }
            }
        }

        let record_positions = records
            .iter()
            .enumerate()
            .map(|(position, record)| (record.id.clone(), position))
            .collect();

        Ok(Self {
            hierarchy,
            records,
            record_positions,
            counts,
        })
    }

    pub fn counts(&self) -> &LocationCounts {
        &self.counts
    }

    pub fn provinces(&self) -> Vec<String> {
        self.hierarchy
            .provinces
            .iter()
            .map(|province| province.name.clone())
            .collect()
    }

    pub fn districts(&self, province_name: &str) -> Vec<String> {
        self.hierarchy
            .provinces
            .iter()
            .find(|province| province.name == province_name)
            .map(|province| {
                province
                    .districts
                    .iter()
                    .map(|district| district.name.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn sectors(&self, province_name: &str, district_name: &str) -> Vec<String> {
        self.hierarchy
            .provinces
            .iter()
            .find(|province| province.name == province_name)
            .and_then(|province| {
                province
                    .districts
                    .iter()
                    .find(|district| district.name == district_name)
            })
            .map(|district| {
                district
                    .sectors
                    .iter()
                    .map(|sector| sector.name.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn cells(
        &self,
        province_name: &str,
        district_name: &str,
        sector_name: &str,
    ) -> Vec<String> {
        self.hierarchy
            .provinces
            .iter()
            .find(|province| province.name == province_name)
            .and_then(|province| {
                province
                    .districts
                    .iter()
                    .find(|district| district.name == district_name)
            })
            .and_then(|district| {
                district
                    .sectors
                    .iter()
                    .find(|sector| sector.name == sector_name)
            })
            .map(|sector| sector.cells.iter().map(|cell| cell.name.clone()).collect())
            .unwrap_or_default()
    }

    pub fn villages(
        &self,
        province_name: &str,
        district_name: &str,
        sector_name: &str,
        cell_name: &str,
    ) -> Vec<String> {
        self.hierarchy
            .provinces
            .iter()
            .find(|province| province.name == province_name)
            .and_then(|province| {
                province
                    .districts
                    .iter()
                    .find(|district| district.name == district_name)
            })
            .and_then(|district| {
                district
                    .sectors
                    .iter()
                    .find(|sector| sector.name == sector_name)
            })
            .and_then(|sector| sector.cells.iter().find(|cell| cell.name == cell_name))
            .map(|cell| {
                cell.villages
                    .iter()
                    .map(|village| village.name.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn search(&self, query: &LocationQuery) -> SearchPage {
        let needle = query.query.trim().to_lowercase();
        let matches: Vec<&LocationRecord> = self
            .records
            .iter()
            .filter(|record| needle.is_empty() || record.search_text.contains(&needle))
            .filter(|record| query.province.is_empty() || record.province == query.province)
            .filter(|record| query.district.is_empty() || record.district == query.district)
            .filter(|record| query.sector.is_empty() || record.sector == query.sector)
            .filter(|record| query.cell.is_empty() || record.cell == query.cell)
            .filter(|record| query.village.is_empty() || record.village == query.village)
            .collect();

        let total = matches.len();
        let total_pages = total.div_ceil(query.per_page).max(1);
        let page = query.page.min(total_pages);
        let start = (page - 1) * query.per_page;
        let results = matches
            .into_iter()
            .skip(start)
            .take(query.per_page)
            .cloned()
            .collect();

        SearchPage {
            results,
            total,
            page,
            total_pages,
            has_previous: page > 1,
            has_next: page < total_pages,
            previous_page: page.saturating_sub(1).max(1),
            next_page: (page + 1).min(total_pages),
        }
    }

    pub fn details(&self, id: &str) -> Option<LocationDetails> {
        let position = *self.record_positions.get(id)?;
        let location = self.records[position].clone();
        let sibling_villages = self.villages(
            &location.province,
            &location.district,
            &location.sector,
            &location.cell,
        );
        Some(LocationDetails {
            location,
            sibling_villages,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_and_indexes_the_real_dataset() {
        let store = LocationStore::load("locations.json").expect("locations.json should load");
        assert_eq!(store.counts().provinces, 5);
        assert_eq!(store.counts().districts, 30);
        assert_eq!(store.counts().sectors, 416);
        assert_eq!(store.counts().cells, 2_148);
        assert_eq!(store.counts().villages, 14_840);
    }

    #[test]
    fn filters_and_paginates_leaf_records() {
        let store = LocationStore::load("locations.json").expect("locations.json should load");
        let query = LocationQuery {
            query: "gihanga".to_owned(),
            province: "Kigali City".to_owned(),
            per_page: 5,
            ..LocationQuery::default()
        }
        .normalized();
        let page = store.search(&query);
        assert!(page.total > 0);
        assert!(page.results.len() <= 5);
        assert!(page.results.iter().all(|location| {
            location.province == "Kigali City" && location.search_text.contains("gihanga")
        }));
    }

    #[test]
    fn resolves_the_full_cascade_and_details() {
        let store = LocationStore::load("locations.json").expect("locations.json should load");
        assert!(store
            .districts("Kigali City")
            .contains(&"Nyarugenge".to_owned()));
        assert!(store
            .sectors("Kigali City", "Nyarugenge")
            .contains(&"Gitega".to_owned()));
        assert!(store
            .cells("Kigali City", "Nyarugenge", "Gitega")
            .contains(&"Akabahizi".to_owned()));
        assert!(store
            .villages("Kigali City", "Nyarugenge", "Gitega", "Akabahizi")
            .contains(&"Gihanga".to_owned()));

        let details = store.details("v00001").expect("first record should exist");
        assert_eq!(
            details.location.breadcrumb,
            "Kigali City / Nyarugenge / Gitega / Akabahizi / Gihanga"
        );
    }
}
