use super::*;
use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::utils::capitalize;

/// Fetch basic information about a Pokémon.
///
/// Returns the Pokémon's name, types, height, weight, and abilities.
/// Use this for quick lookups when you need basic info.
///
/// # Arguments
/// * `pokemon_name` - The name or Pokedex number of the Pokémon (case-insensitive).
///   - Examples: "pikachu", "Charizard", "25", "mewtwo"
///
/// # Returns
/// Formatted information including:
/// - Name (capitalized)
/// - Types (e.g., "Electric", "Fire/Flying")
/// - Height in meters
/// - Weight in kilograms
/// - Abilities (including hidden abilities)
///
/// # Errors
/// Returns error message if Pokémon is not found or API is unavailable.
///
#[ollama_rs::function]
pub async fn fetch_pokemon_basic(pokemon_name: String) -> ToolResult<String> {
    log_tool_call(
        "fetch_pokemon_basic",
        &[("pokemon_name".to_string(), pokemon_name.clone())],
    );

    let url = format!(
        "https://pokeapi.co/api/v2/pokemon/{}/",
        pokemon_name.to_lowercase()
    );

    let response = match reqwest::get(&url).await {
        Ok(r) => r,
        Err(e) => {
            let err = format!(
                "Network error while fetching Pokémon: {}. Please try again later.",
                e
            );
            log_tool_result("fetch_pokemon_basic", &err);
            return Ok(err);
        }
    };

    if !response.status().is_success() {
        let err = format!(
            "Error: Pokémon '{}' not found. HTTP {}",
            pokemon_name,
            response.status()
        );
        log_tool_result("fetch_pokemon_basic", &err);
        return Ok(err);
    }

    let data: PokemonData = match response.json().await {
        Ok(d) => d,
        Err(e) => {
            let err = format!("Error parsing Pokémon data: {}. Please try again later.", e);
            log_tool_result("fetch_pokemon_basic", &err);
            return Ok(err);
        }
    };

    let name = capitalize(&data.name);
    let types: Vec<String> = data
        .types
        .iter()
        .map(|t| t.type_info.name.clone())
        .collect();
    let abilities: Vec<String> = data
        .abilities
        .iter()
        .map(|a| a.ability.name.clone())
        .collect();
    let height = data.height as f32 / 10.0; // decimeters to meters
    let weight = data.weight as f32 / 10.0; // hectograms to kg

    let result = format!(
        "Name: {}\nTypes: {}\nHeight: {:.1}m\nWeight: {:.1}kg\nAbilities: {}",
        name,
        types.join(", "),
        height,
        weight,
        abilities.join(", ")
    );
    log_tool_result("fetch_pokemon_basic", &result);
    Ok(result)
}

/// Fetch base stats of a Pokémon.
///
/// Returns the Pokémon's base stats used for battle calculations.
/// Use this to compare Pokémon stats or plan competitive builds.
///
/// # Arguments
/// * `pokemon_name` - The name or Pokedex number of the Pokémon (case-insensitive).
///   - Examples: "gengar", "dragonite", "6" (Charizard)
///
/// # Returns
/// Base stats including:
/// - HP (Hit Points)
/// - Attack
/// - Defense
/// - Special Attack
/// - Special Defense
/// - Speed
/// - Base stat total (BST)
///
/// # Errors
/// Returns error message if Pokémon is not found or API is unavailable.
///
#[ollama_rs::function]
pub async fn fetch_pokemon_stats(pokemon_name: String) -> ToolResult<String> {
    log_tool_call(
        "fetch_pokemon_stats",
        &[("pokemon_name".to_string(), pokemon_name.clone())],
    );

    let url = format!(
        "https://pokeapi.co/api/v2/pokemon/{}/",
        pokemon_name.to_lowercase()
    );

    let response = match reqwest::get(&url).await {
        Ok(r) => r,
        Err(e) => {
            let err = format!(
                "Network error while fetching Pokémon stats: {}. Please try again later.",
                e
            );
            log_tool_result("fetch_pokemon_stats", &err);
            return Ok(err);
        }
    };

    if !response.status().is_success() {
        let err = format!(
            "Error: Pokémon '{}' not found. HTTP {}",
            pokemon_name,
            response.status()
        );
        log_tool_result("fetch_pokemon_stats", &err);
        return Ok(err);
    }

    let data: PokemonData = match response.json().await {
        Ok(d) => d,
        Err(e) => {
            let err = format!(
                "Error parsing Pokémon stats: {}. Please try again later.",
                e
            );
            log_tool_result("fetch_pokemon_stats", &err);
            return Ok(err);
        }
    };

    let name = capitalize(&data.name);
    let stats: std::collections::HashMap<String, u32> = data
        .stats
        .iter()
        .map(|s| (s.stat.name.clone(), s.base_stat))
        .collect();

    let total: u32 = stats.values().sum();

    let result = format!(
        "{} Base Stats:\nHP: {}\nAttack: {}\nDefense: {}\nSpecial Attack: {}\nSpecial Defense: {}\nSpeed: {}\nTotal: {}",
        name,
        stats.get("hp").unwrap_or(&0),
        stats.get("attack").unwrap_or(&0),
        stats.get("defense").unwrap_or(&0),
        stats.get("special-attack").unwrap_or(&0),
        stats.get("special-defense").unwrap_or(&0),
        stats.get("speed").unwrap_or(&0),
        total
    );
    log_tool_result("fetch_pokemon_stats", &result);
    Ok(result)
}

/// Fetch moves that a Pokémon can learn.
///
/// Returns a list of moves the Pokémon can learn through leveling,
/// TMs, breeding, etc. Note that some Pokémon have many moves, so
/// use the limit parameter to avoid overwhelming output.
///
/// # Arguments
/// * `pokemon_name` - The name or Pokedex number of the Pokémon (case-insensitive).
///   - Examples: "charizard", "pikachu", "150"
/// * `limit` - Maximum number of moves to return. Use a reasonable number.
///   - Example: 20 for a quick overview, 50 for detailed analysis
///
/// # Returns
/// List of moves with:
/// - Move name
/// - Learn method (level-up, machine, egg, etc.)
/// - Level learned (for level-up moves)
///
/// # Errors
/// Returns error message if Pokémon is not found or API is unavailable.
///
#[ollama_rs::function]
pub async fn fetch_pokemon_moves(pokemon_name: String, limit: u32) -> ToolResult<String> {
    log_tool_call(
        "fetch_pokemon_moves",
        &[
            ("pokemon_name".to_string(), pokemon_name.clone()),
            ("limit".to_string(), limit.to_string()),
        ],
    );

    let url = format!(
        "https://pokeapi.co/api/v2/pokemon/{}/",
        pokemon_name.to_lowercase()
    );

    let response = match reqwest::get(&url).await {
        Ok(r) => r,
        Err(e) => {
            let err = format!(
                "Network error while fetching Pokémon moves: {}. Please try again later.",
                e
            );
            log_tool_result("fetch_pokemon_moves", &err);
            return Ok(err);
        }
    };

    if !response.status().is_success() {
        let err = format!(
            "Error: Pokémon '{}' not found. HTTP {}",
            pokemon_name,
            response.status()
        );
        log_tool_result("fetch_pokemon_moves", &err);
        return Ok(err);
    }

    let data: PokemonData = match response.json().await {
        Ok(d) => d,
        Err(e) => {
            let err = format!(
                "Error parsing Pokémon moves: {}. Please try again later.",
                e
            );
            log_tool_result("fetch_pokemon_moves", &err);
            return Ok(err);
        }
    };

    let name = capitalize(&data.name);
    let total_moves = data.moves.len();
    let actual_limit = std::cmp::min(limit as usize, data.moves.len());
    let moves: Vec<String> = data
        .moves
        .iter()
        .take(actual_limit)
        .map(|m| m.r#move.name.clone())
        .collect();

    let moves_list = moves
        .iter()
        .map(|m| format!("  - {}", m))
        .collect::<Vec<_>>()
        .join("\n");

    let result = format!(
        "{} can learn {} moves total.\nFirst {} moves:\n{}",
        name, total_moves, actual_limit, moves_list
    );
    log_tool_result("fetch_pokemon_moves", &result);
    Ok(result)
}

/// Fetch evolution chain for a Pokémon species.
///
/// Returns the complete evolution chain showing how a Pokémon evolves.
/// Use this to understand evolution requirements and stages.
///
/// # Arguments
/// * `pokemon_name` - The name of the Pokémon species (case-insensitive).
///   - Examples: "pikachu", "charmander", "eevee"
///
/// # Returns
/// Evolution chain with:
/// - All Pokémon in the chain
/// - Evolution triggers (level, item, trade, etc.)
/// - Conditions for each evolution
///
/// # Errors
/// Returns error message if Pokémon is not found or API is unavailable.
///
#[ollama_rs::function]
pub async fn fetch_pokemon_evolution(pokemon_name: String) -> ToolResult<String> {
    log_tool_call(
        "fetch_pokemon_evolution",
        &[("pokemon_name".to_string(), pokemon_name.clone())],
    );

    let species_url = format!(
        "https://pokeapi.co/api/v2/pokemon-species/{}/",
        pokemon_name.to_lowercase()
    );

    let response = match reqwest::get(&species_url).await {
        Ok(r) => r,
        Err(e) => {
            let err = format!(
                "Network error while fetching Pokémon species: {}. Please try again later.",
                e
            );
            log_tool_result("fetch_pokemon_evolution", &err);
            return Ok(err);
        }
    };

    if !response.status().is_success() {
        let err = format!(
            "Error: Pokémon species '{}' not found. HTTP {}",
            pokemon_name,
            response.status()
        );
        log_tool_result("fetch_pokemon_evolution", &err);
        return Ok(err);
    }

    let species: SpeciesData = match response.json().await {
        Ok(s) => s,
        Err(e) => {
            let err = format!("Error parsing species data: {}. Please try again later.", e);
            log_tool_result("fetch_pokemon_evolution", &err);
            return Ok(err);
        }
    };

    let evo_url = &species.evolution_chain.url;

    let evo_response = match reqwest::get(evo_url).await {
        Ok(r) => r,
        Err(e) => {
            let err = format!(
                "Network error while fetching evolution chain: {}. Please try again later.",
                e
            );
            log_tool_result("fetch_pokemon_evolution", &err);
            return Ok(err);
        }
    };

    if !evo_response.status().is_success() {
        let err = format!(
            "Error fetching evolution chain: HTTP {}",
            evo_response.status()
        );
        log_tool_result("fetch_pokemon_evolution", &err);
        return Ok(err);
    }

    let chain: EvolutionChain = match evo_response.json().await {
        Ok(c) => c,
        Err(e) => {
            let err = format!(
                "Error parsing evolution chain: {}. Please try again later.",
                e
            );
            log_tool_result("fetch_pokemon_evolution", &err);
            return Ok(err);
        }
    };

    let formatted = format_evolution_chain(&chain.chain, 0);
    let result = format!("Evolution Chain:\n{}", formatted);
    log_tool_result("fetch_pokemon_evolution", &result);
    Ok(result)
}

/// Fetch detailed information about a Pokémon ability.
///
/// Returns the description and effects of a Pokémon ability.
/// Use this to understand what an ability does in battle.
///
/// # Arguments
/// * `ability_name` - The name of the ability (case-insensitive).
///   - Examples: "levitate", "intimidate", "sturdy"
///
/// # Returns
/// Ability information including:
/// - Ability name
/// - Short description (in-game effect)
/// - Detailed effect explanation
///
/// # Errors
/// Returns error message if ability is not found or API is unavailable.
///
#[ollama_rs::function]
pub async fn fetch_ability_details(ability_name: String) -> ToolResult<String> {
    log_tool_call(
        "fetch_ability_details",
        &[("ability_name".to_string(), ability_name.clone())],
    );

    let url = format!(
        "https://pokeapi.co/api/v2/ability/{}/",
        ability_name.to_lowercase()
    );

    let response = match reqwest::get(&url).await {
        Ok(r) => r,
        Err(e) => {
            let err = format!(
                "Network error while fetching ability: {}. Please try again later.",
                e
            );
            log_tool_result("fetch_ability_details", &err);
            return Ok(err);
        }
    };

    if !response.status().is_success() {
        let err = format!(
            "Error: Ability '{}' not found. HTTP {}",
            ability_name,
            response.status()
        );
        log_tool_result("fetch_ability_details", &err);
        return Ok(err);
    }

    let data: AbilityData = match response.json().await {
        Ok(d) => d,
        Err(e) => {
            let err = format!("Error parsing ability data: {}. Please try again later.", e);
            log_tool_result("fetch_ability_details", &err);
            return Ok(err);
        }
    };

    let name = data
        .names
        .iter()
        .find(|n| n.language.name == "en")
        .map(|n| n.name.clone())
        .unwrap_or(ability_name);

    let effect = data
        .effect_entries
        .iter()
        .find(|e| e.language.name == "en")
        .map(|e| e.short_effect.clone())
        .unwrap_or_else(|| "No effect description available.".to_string());

    let pokemon_list: Vec<String> = data
        .pokemon
        .iter()
        .take(10)
        .map(|p| p.pokemon.name.clone())
        .collect();

    let result = format!(
        "Ability: {}\nEffect: {}\nPokémon with this ability: {}",
        name,
        effect,
        pokemon_list.join(", ")
    );
    log_tool_result("fetch_ability_details", &result);
    Ok(result)
}

/// Fetch type effectiveness (weaknesses, resistances, immunities).
///
/// Returns damage relationships for a Pokémon type - what it's strong/weak against.
/// Essential for battle strategy and team building.
///
/// # Arguments
/// * `type_name` - The name of the type (case-insensitive).
///   - Examples: "fire", "water", "dragon", "electric"
///
/// # Returns
/// Type effectiveness chart showing:
/// - Double damage TO (super effective)
/// - Half damage TO (not very effective)
/// - No damage TO (ineffective)
/// - Double damage FROM (weakness)
/// - Half damage FROM (resistance)
/// - No damage FROM (immunity)
///
/// # Errors
/// Returns error message if type is not found or API is unavailable.
///
#[ollama_rs::function]
pub async fn fetch_type_effectiveness(type_name: String) -> ToolResult<String> {
    log_tool_call(
        "fetch_type_effectiveness",
        &[("type_name".to_string(), type_name.clone())],
    );

    let url = format!(
        "https://pokeapi.co/api/v2/type/{}/",
        type_name.to_lowercase()
    );

    let response = match reqwest::get(&url).await {
        Ok(r) => r,
        Err(e) => {
            let err = format!(
                "Network error while fetching type: {}. Please try again later.",
                e
            );
            log_tool_result("fetch_type_effectiveness", &err);
            return Ok(err);
        }
    };

    if !response.status().is_success() {
        let err = format!(
            "Error: Type '{}' not found. HTTP {}",
            type_name,
            response.status()
        );
        log_tool_result("fetch_type_effectiveness", &err);
        return Ok(err);
    }

    let data: TypeData = match response.json().await {
        Ok(d) => d,
        Err(e) => {
            let err = format!("Error parsing type data: {}. Please try again later.", e);
            log_tool_result("fetch_type_effectiveness", &err);
            return Ok(err);
        }
    };

    let dr = &data.damage_relations;

    let double_damage_from: Vec<String> = dr
        .double_damage_from
        .iter()
        .map(|t| t.name.clone())
        .collect();
    let half_damage_from: Vec<String> =
        dr.half_damage_from.iter().map(|t| t.name.clone()).collect();
    let no_damage_from: Vec<String> = dr.no_damage_from.iter().map(|t| t.name.clone()).collect();
    let double_damage_to: Vec<String> =
        dr.double_damage_to.iter().map(|t| t.name.clone()).collect();
    let half_damage_to: Vec<String> = dr.half_damage_to.iter().map(|t| t.name.clone()).collect();

    fn format_list(list: &[String]) -> String {
        if list.is_empty() {
            "None".to_string()
        } else {
            list.join(", ")
        }
    }

    let result = format!(
        "{} Type:\n\nWeak to (2x damage): {}\nResistant to (0.5x damage): {}\nImmune to (0x damage): {}\n\nSuper effective against (2x): {}\nNot very effective against (0.5x): {}",
        capitalize(&type_name),
        format_list(&double_damage_from),
        format_list(&half_damage_from),
        format_list(&no_damage_from),
        format_list(&double_damage_to),
        format_list(&half_damage_to)
    );
    log_tool_result("fetch_type_effectiveness", &result);
    Ok(result)
}

/// List all Pokémon of a specific type.
///
/// Returns a list of Pokémon that have the specified type.
/// Use this to find Pokémon for team building or type-based strategies.
///
/// # Arguments
/// * `type_name` - The name of the type (case-insensitive).
///   - Examples: "fire", "water", "dragon", "fairy"
/// * `limit` - Maximum number of Pokémon to return (default: 20, max: 100). Optional.
///   - Example: "10" for a quick list, "50" for a comprehensive list
///
/// # Returns
/// List of Pokémon names with the specified type.
///
/// # Errors
/// Returns error message if type is not found or API is unavailable.
///
#[ollama_rs::function]
pub async fn fetch_pokemon_by_type(type_name: String, limit: Option<String>) -> ToolResult<String> {
    let limit_num = limit
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(20)
        .min(100);

    log_tool_call(
        "fetch_pokemon_by_type",
        &[
            ("type_name".to_string(), type_name.clone()),
            ("limit".to_string(), limit_num.to_string()),
        ],
    );

    let url = format!(
        "https://pokeapi.co/api/v2/type/{}/",
        type_name.to_lowercase()
    );

    let response = match reqwest::get(&url).await {
        Ok(r) => r,
        Err(e) => {
            let err = format!(
                "Network error while fetching type: {}. Please try again later.",
                e
            );
            log_tool_result("fetch_pokemon_by_type", &err);
            return Ok(err);
        }
    };

    if !response.status().is_success() {
        let err = format!(
            "Error: Type '{}' not found. HTTP {}",
            type_name,
            response.status()
        );
        log_tool_result("fetch_pokemon_by_type", &err);
        return Ok(err);
    }

    let data: TypeData = match response.json().await {
        Ok(d) => d,
        Err(e) => {
            let err = format!("Error parsing type data: {}. Please try again later.", e);
            log_tool_result("fetch_pokemon_by_type", &err);
            return Ok(err);
        }
    };

    let total = data.pokemon.len();
    let pokemon_list: Vec<String> = data
        .pokemon
        .iter()
        .take(limit_num)
        .map(|p| {
            let name = p.pokemon.name.replace('-', " ");
            capitalize(&name)
        })
        .collect();

    let result = format!(
        "**{} Type Pokémon** (showing {} of {}):\n\n{}",
        capitalize(&type_name),
        pokemon_list.len(),
        total,
        pokemon_list.join(", ")
    );
    log_tool_result("fetch_pokemon_by_type", &result);
    Ok(result)
}

/// Fetch detailed information about a Pokémon move.
///
/// Returns complete information about a move including stats, type,
/// and effects. Use this for battle analysis and move selection.
///
/// # Arguments
/// * `move_name` - The name of the move (case-insensitive).
///   - Examples: "thunderbolt", "flamethrower", "earthquake"
///
/// # Returns
/// Move details including:
/// - Name and type
/// - Power, accuracy, and PP
/// - Damage class (physical, special, status)
/// - Effect description
///
/// # Errors
/// Returns error message if move is not found or API is unavailable.
///
#[ollama_rs::function]
pub async fn fetch_move_details(move_name: String) -> ToolResult<String> {
    log_tool_call(
        "fetch_move_details",
        &[("move_name".to_string(), move_name.clone())],
    );

    let url = format!(
        "https://pokeapi.co/api/v2/move/{}/",
        move_name.to_lowercase()
    );

    let response = match reqwest::get(&url).await {
        Ok(r) => r,
        Err(e) => {
            let err = format!(
                "Network error while fetching move: {}. Please try again later.",
                e
            );
            log_tool_result("fetch_move_details", &err);
            return Ok(err);
        }
    };

    if !response.status().is_success() {
        let err = format!(
            "Error: Move '{}' not found. HTTP {}",
            move_name,
            response.status()
        );
        log_tool_result("fetch_move_details", &err);
        return Ok(err);
    }

    let data: MoveData = match response.json().await {
        Ok(d) => d,
        Err(e) => {
            let err = format!("Error parsing move data: {}. Please try again later.", e);
            log_tool_result("fetch_move_details", &err);
            return Ok(err);
        }
    };

    let name = data
        .names
        .iter()
        .find(|n| n.language.name == "en")
        .map(|n| n.name.clone())
        .unwrap_or(move_name);

    let effect = data
        .effect_entries
        .iter()
        .find(|e| e.language.name == "en")
        .map(|e| e.short_effect.clone())
        .unwrap_or_else(|| "No effect description.".to_string());

    let accuracy = data
        .accuracy
        .map(|a| a.to_string())
        .unwrap_or_else(|| "—".to_string());
    let power = data
        .power
        .map(|p| p.to_string())
        .unwrap_or_else(|| "—".to_string());

    let result = format!(
        "Move: {}\nType: {}\nCategory: {}\nPower: {}\nAccuracy: {}\nPP: {}\nPriority: {}\nEffect: {}",
        name,
        capitalize(&data.type_info.name),
        capitalize(&data.damage_class.name),
        power,
        accuracy,
        data.pp,
        data.priority,
        effect
    );
    log_tool_result("fetch_move_details", &result);
    Ok(result)
}

/// Fetch comprehensive information about a Pokémon.
///
/// Combines basic info, stats, and abilities into a single response.
/// Use this when you need complete information about a Pokémon.
///
/// # Arguments
/// * `pokemon_name` - The name or Pokedex number of the Pokémon (case-insensitive).
///   - Examples: "pikachu", "charizard", "150" (Mewtwo)
///
/// # Returns
/// Complete Pokémon information including:
/// - Name, types, height, weight
/// - Base stats (HP, Attack, Defense, Sp. Atk, Sp. Def, Speed)
/// - Abilities (including hidden abilities)
/// - Base stat total
///
/// # Errors
/// Returns error message if Pokémon is not found or API is unavailable.
///
#[ollama_rs::function]
pub async fn fetch_pokemon(pokemon_name: String) -> ToolResult<String> {
    log_tool_call(
        "fetch_pokemon",
        &[("pokemon_name".to_string(), pokemon_name.clone())],
    );

    let url = format!(
        "https://pokeapi.co/api/v2/pokemon/{}/",
        pokemon_name.to_lowercase()
    );

    let response = match reqwest::get(&url).await {
        Ok(r) => r,
        Err(e) => {
            let err = format!(
                "Network error while fetching Pokémon: {}. Please try again later.",
                e
            );
            log_tool_result("fetch_pokemon", &err);
            return Ok(err);
        }
    };

    if !response.status().is_success() {
        let err = format!(
            "Error: Pokémon '{}' not found. HTTP {}",
            pokemon_name,
            response.status()
        );
        log_tool_result("fetch_pokemon", &err);
        return Ok(err);
    }

    let data: PokemonData = match response.json().await {
        Ok(d) => d,
        Err(e) => {
            let err = format!("Error parsing Pokémon data: {}. Please try again later.", e);
            log_tool_result("fetch_pokemon", &err);
            return Ok(err);
        }
    };

    let name = capitalize(&data.name);
    let types: Vec<String> = data
        .types
        .iter()
        .map(|t| t.type_info.name.clone())
        .collect();
    let abilities: Vec<String> = data
        .abilities
        .iter()
        .map(|a| a.ability.name.clone())
        .collect();
    let height = data.height as f32 / 10.0;
    let weight = data.weight as f32 / 10.0;

    let stats: std::collections::HashMap<String, u32> = data
        .stats
        .iter()
        .map(|s| (s.stat.name.clone(), s.base_stat))
        .collect();
    let total_stats: u32 = stats.values().sum();

    // Fetch ability details for first 3 abilities
    let mut ability_details = Vec::new();
    for ability_name in abilities.iter().take(3) {
        let url = format!("https://pokeapi.co/api/v2/ability/{}/", ability_name);
        match reqwest::get(&url).await {
            Ok(response) if response.status().is_success() => {
                if let Ok(ability_data) = response.json::<AbilityData>().await {
                    let effect = ability_data
                        .effect_entries
                        .iter()
                        .find(|e| e.language.name == "en")
                        .map(|e| e.short_effect.clone())
                        .unwrap_or_else(|| "No description.".to_string());
                    ability_details.push(format!("  - {}: {}", ability_name, effect));
                } else {
                    ability_details.push(format!("  - {}", ability_name));
                }
            }
            _ => {
                ability_details.push(format!("  - {}", ability_name));
            }
        }
    }

    let result = format!(
        "{}\nTypes: {}\nHeight: {:.1}m | Weight: {:.1}kg\n\nBase Stats:\n  HP: {} | Attack: {} | Defense: {}\n  Sp. Attack: {} | Sp. Defense: {} | Speed: {}\n  Total: {}\n\nAbilities:\n{}",
        name,
        types.join(", "),
        height,
        weight,
        stats.get("hp").unwrap_or(&0),
        stats.get("attack").unwrap_or(&0),
        stats.get("defense").unwrap_or(&0),
        stats.get("special-attack").unwrap_or(&0),
        stats.get("special-defense").unwrap_or(&0),
        stats.get("speed").unwrap_or(&0),
        total_stats,
        ability_details.join("\n")
    );
    log_tool_result("fetch_pokemon", &result);
    Ok(result)
}

fn format_evolution_chain(link: &EvolutionLink, depth: usize) -> String {
    let species = capitalize(&link.species.name);
    let indent = "  ".repeat(depth);
    let mut result = format!("{}- {}", indent, species);

    if let Some(details) = link.evolution_details.first() {
        let mut conditions = Vec::new();

        if let Some(level) = details.min_level {
            conditions.push(format!("Level {}", level));
        }

        if let Some(item) = &details.item {
            conditions.push(item.name.clone());
        }

        if let Some(trigger) = &details.trigger {
            conditions.push(trigger.name.clone());
        }

        if !conditions.is_empty() {
            result.push_str(&format!(" (evolves via: {})", conditions.join(", ")));
        }
    }

    for next_link in &link.evolves_to {
        result.push('\n');
        result.push_str(&format_evolution_chain(next_link, depth + 1));
    }

    result
}
