use serde::Deserialize;

pub mod pokemon;
pub mod search;
pub mod weather;

pub use pokemon::*;
pub use search::*;
pub use weather::*;

/// Common response structure for PokeAPI
#[derive(Debug, Deserialize)]
pub struct NamedApiResource {
    pub name: String,
    #[allow(dead_code)]
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct NameEntry {
    pub name: String,
    pub language: NamedApiResource,
}

#[derive(Debug, Deserialize)]
pub struct EffectEntry {
    pub short_effect: String,
    pub language: NamedApiResource,
}

#[derive(Debug, Deserialize)]
pub struct PokemonSlot {
    pub pokemon: NamedApiResource,
}

#[derive(Debug, Deserialize)]
pub struct StatEntry {
    pub base_stat: u32,
    pub stat: NamedApiResource,
}

#[derive(Debug, Deserialize)]
pub struct TypeSlot {
    #[serde(rename = "type")]
    pub type_info: NamedApiResource,
}

#[derive(Debug, Deserialize)]
pub struct AbilitySlot {
    pub ability: NamedApiResource,
}

#[derive(Debug, Deserialize)]
pub struct MoveSlot {
    pub r#move: NamedApiResource,
}

#[derive(Debug, Deserialize)]
pub struct PokemonData {
    pub name: String,
    pub height: u32,
    pub weight: u32,
    pub types: Vec<TypeSlot>,
    pub abilities: Vec<AbilitySlot>,
    pub stats: Vec<StatEntry>,
    pub moves: Vec<MoveSlot>,
}

#[derive(Debug, Deserialize)]
pub struct AbilityData {
    pub names: Vec<NameEntry>,
    pub effect_entries: Vec<EffectEntry>,
    pub pokemon: Vec<PokemonSlot>,
}

#[derive(Debug, Deserialize)]
pub struct TypeData {
    #[serde(rename = "damage_relations")]
    pub damage_relations: DamageRelations,
}

#[derive(Debug, Deserialize)]
pub struct DamageRelations {
    pub double_damage_from: Vec<NamedApiResource>,
    pub half_damage_from: Vec<NamedApiResource>,
    pub no_damage_from: Vec<NamedApiResource>,
    pub double_damage_to: Vec<NamedApiResource>,
    pub half_damage_to: Vec<NamedApiResource>,
}

#[derive(Debug, Deserialize)]
pub struct MoveData {
    pub names: Vec<NameEntry>,
    pub effect_entries: Vec<EffectEntry>,
    #[serde(rename = "type")]
    pub type_info: NamedApiResource,
    pub damage_class: NamedApiResource,
    pub power: Option<u32>,
    pub accuracy: Option<u32>,
    pub pp: u32,
    pub priority: i32,
}

#[derive(Debug, Deserialize)]
pub struct SpeciesData {
    #[serde(rename = "evolution_chain")]
    pub evolution_chain: EvolutionChainUrl,
}

#[derive(Debug, Deserialize)]
pub struct EvolutionChainUrl {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct EvolutionChain {
    pub chain: EvolutionLink,
}

#[derive(Debug, Deserialize)]
pub struct EvolutionLink {
    pub species: NamedApiResource,
    #[serde(rename = "evolution_details")]
    pub evolution_details: Vec<EvolutionDetail>,
    #[serde(rename = "evolves_to")]
    pub evolves_to: Vec<EvolutionLink>,
}

#[derive(Debug, Deserialize)]
pub struct EvolutionDetail {
    pub min_level: Option<u32>,
    pub item: Option<NamedApiResource>,
    pub trigger: Option<NamedApiResource>,
}
