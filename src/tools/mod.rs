// Pokemon data structures only available when pokemon-tools feature is enabled
#[cfg(feature = "pokemon-tools")]
use serde::Deserialize;

#[cfg(feature = "calc-tools")]
pub mod calc;
pub mod files;
#[cfg(feature = "finance-tools")]
pub mod finance;
pub mod misc;
#[cfg(feature = "pokemon-tools")]
pub mod pokemon;
pub mod registry;
#[cfg(feature = "search-tools")]
pub mod search_builtin;
#[cfg(feature = "serper-tools")]
pub mod serper;
#[cfg(feature = "system-tools")]
pub mod system;
pub mod weather;

#[cfg(feature = "calc-tools")]
pub use calc::*;
pub use files::*;
#[cfg(feature = "finance-tools")]
pub use finance::*;
pub use misc::*;
#[cfg(feature = "pokemon-tools")]
pub use pokemon::*;
pub use registry::{get_available_tool_names, register_tools};
#[cfg(feature = "search-tools")]
pub use search_builtin::*;
#[cfg(feature = "system-tools")]
pub use system::*;
pub use weather::*;

/// Common response structure for PokeAPI
#[cfg(feature = "pokemon-tools")]
#[derive(Debug, Deserialize)]
pub struct NamedApiResource {
    pub name: String,
    /// URL is needed for JSON deserialization and is used in pokemon.rs
    #[allow(dead_code)]
    pub url: String,
}

#[cfg(feature = "pokemon-tools")]
#[derive(Debug, Deserialize)]
pub struct NameEntry {
    pub name: String,
    pub language: NamedApiResource,
}

#[cfg(feature = "pokemon-tools")]
#[derive(Debug, Deserialize)]
pub struct EffectEntry {
    pub short_effect: String,
    pub language: NamedApiResource,
}

#[cfg(feature = "pokemon-tools")]
#[derive(Debug, Deserialize)]
pub struct PokemonSlot {
    pub pokemon: NamedApiResource,
}

#[cfg(feature = "pokemon-tools")]
#[derive(Debug, Deserialize)]
pub struct StatEntry {
    pub base_stat: u32,
    pub stat: NamedApiResource,
}

#[cfg(feature = "pokemon-tools")]
#[derive(Debug, Deserialize)]
pub struct TypeSlot {
    #[serde(rename = "type")]
    pub type_info: NamedApiResource,
}

#[cfg(feature = "pokemon-tools")]
#[derive(Debug, Deserialize)]
pub struct AbilitySlot {
    pub ability: NamedApiResource,
}

#[cfg(feature = "pokemon-tools")]
#[derive(Debug, Deserialize)]
pub struct MoveSlot {
    pub r#move: NamedApiResource,
}

#[cfg(feature = "pokemon-tools")]
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

#[cfg(feature = "pokemon-tools")]
#[derive(Debug, Deserialize)]
pub struct AbilityData {
    pub names: Vec<NameEntry>,
    pub effect_entries: Vec<EffectEntry>,
    pub pokemon: Vec<PokemonSlot>,
}

#[cfg(feature = "pokemon-tools")]
#[derive(Debug, Deserialize)]
pub struct TypeData {
    #[serde(rename = "damage_relations")]
    pub damage_relations: DamageRelations,
    pub pokemon: Vec<TypePokemonSlot>,
}

#[cfg(feature = "pokemon-tools")]
#[derive(Debug, Deserialize)]
pub struct TypePokemonSlot {
    pub pokemon: NamedApiResource,
}

#[cfg(feature = "pokemon-tools")]
#[derive(Debug, Deserialize)]
pub struct DamageRelations {
    pub double_damage_from: Vec<NamedApiResource>,
    pub half_damage_from: Vec<NamedApiResource>,
    pub no_damage_from: Vec<NamedApiResource>,
    pub double_damage_to: Vec<NamedApiResource>,
    pub half_damage_to: Vec<NamedApiResource>,
}

#[cfg(feature = "pokemon-tools")]
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

#[cfg(feature = "pokemon-tools")]
#[derive(Debug, Deserialize)]
pub struct SpeciesData {
    #[serde(rename = "evolution_chain")]
    pub evolution_chain: EvolutionChainUrl,
}

#[cfg(feature = "pokemon-tools")]
#[derive(Debug, Deserialize)]
pub struct EvolutionChainUrl {
    pub url: String,
}

#[cfg(feature = "pokemon-tools")]
#[derive(Debug, Deserialize)]
pub struct EvolutionChain {
    pub chain: EvolutionLink,
}

#[cfg(feature = "pokemon-tools")]
#[derive(Debug, Deserialize)]
pub struct EvolutionLink {
    pub species: NamedApiResource,
    #[serde(rename = "evolution_details")]
    pub evolution_details: Vec<EvolutionDetail>,
    #[serde(rename = "evolves_to")]
    pub evolves_to: Vec<EvolutionLink>,
}

#[cfg(feature = "pokemon-tools")]
#[derive(Debug, Deserialize)]
pub struct EvolutionDetail {
    pub min_level: Option<u32>,
    pub item: Option<NamedApiResource>,
    pub trigger: Option<NamedApiResource>,
}
