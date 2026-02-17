use super::*;

/// Fetch basic information about a Pokémon (name, types, height, weight, abilities).
///
/// * pokemon_name - The name of the Pokémon in lowercase.
#[ollama_rs::function]
pub async fn fetch_pokemon_basic(pokemon_name: String) -> ToolResult<String> {

    let url = format!(
        "https://pokeapi.co/api/v2/pokemon/{}/",
        pokemon_name.to_lowercase()
    );
    let response = reqwest::get(&url).await?;

    if !response.status().is_success() {
        return Ok(format!("Error: HTTP {}", response.status()));
    }

    let data: PokemonData = response.json().await?;
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

    Ok(format!(
        "Name: {}\nTypes: {}\nHeight: {:.1}m\nWeight: {:.1}kg\nAbilities: {}",
        name,
        types.join(", "),
        height,
        weight,
        abilities.join(", ")
    ))
}

/// Fetch base stats of a Pokémon (HP, Attack, Defense, etc.).
///
/// * pokemon_name - The name of the Pokémon in lowercase.
#[ollama_rs::function]
pub async fn fetch_pokemon_stats(pokemon_name: String) -> ToolResult<String> {

    let url = format!(
        "https://pokeapi.co/api/v2/pokemon/{}/",
        pokemon_name.to_lowercase()
    );
    let response = reqwest::get(&url).await?;

    if !response.status().is_success() {
        return Ok(format!("Error: HTTP {}", response.status()));
    }

    let data: PokemonData = response.json().await?;
    let name = capitalize(&data.name);
    let stats: std::collections::HashMap<String, u32> = data
        .stats
        .iter()
        .map(|s| (s.stat.name.clone(), s.base_stat))
        .collect();

    let total: u32 = stats.values().sum();

    Ok(format!(
        "{} Base Stats:\nHP: {}\nAttack: {}\nDefense: {}\nSpecial Attack: {}\nSpecial Defense: {}\nSpeed: {}\nTotal: {}",
        name,
        stats.get("hp").unwrap_or(&0),
        stats.get("attack").unwrap_or(&0),
        stats.get("defense").unwrap_or(&0),
        stats.get("special-attack").unwrap_or(&0),
        stats.get("special-defense").unwrap_or(&0),
        stats.get("speed").unwrap_or(&0),
        total
    ))
}

/// Fetch moves that a Pokémon can learn.
///
/// * pokemon_name - The name of the Pokémon in lowercase.
/// * limit - Maximum number of moves to return (default 20).
#[ollama_rs::function]
pub async fn fetch_pokemon_moves(pokemon_name: String, limit: u32) -> ToolResult<String> {

    let url = format!(
        "https://pokeapi.co/api/v2/pokemon/{}/",
        pokemon_name.to_lowercase()
    );
    let response = reqwest::get(&url).await?;

    if !response.status().is_success() {
        return Ok(format!("Error: HTTP {}", response.status()));
    }

    let data: PokemonData = response.json().await?;
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

    Ok(format!(
        "{} can learn {} moves total.\nFirst {} moves:\n{}",
        name, total_moves, actual_limit, moves_list
    ))
}

/// Fetch evolution chain for a Pokémon species.
///
/// * pokemon_name - The name of the Pokémon species in lowercase.
#[ollama_rs::function]
pub async fn fetch_pokemon_evolution(pokemon_name: String) -> ToolResult<String> {

    let species_url = format!(
        "https://pokeapi.co/api/v2/pokemon-species/{}/",
        pokemon_name.to_lowercase()
    );
    let response = reqwest::get(&species_url).await?;

    if !response.status().is_success() {
        return Ok(format!("Error: HTTP {}", response.status()));
    }

    let species: SpeciesData = response.json().await?;
    let evo_url = &species.evolution_chain.url;

    let evo_response = reqwest::get(evo_url).await?;

    if !evo_response.status().is_success() {
        return Ok(format!(
            "Error fetching evolution chain: HTTP {}",
            evo_response.status()
        ));
    }

    let chain: EvolutionChain = evo_response.json().await?;
    let formatted = format_evolution_chain(&chain.chain, 0);

    Ok(format!("Evolution Chain:\n{}", formatted))
}

/// Fetch detailed information about a Pokémon ability.
///
/// * ability_name - The name of the ability in lowercase.
#[ollama_rs::function]
pub async fn fetch_ability_details(ability_name: String) -> ToolResult<String> {

    let url = format!(
        "https://pokeapi.co/api/v2/ability/{}/",
        ability_name.to_lowercase()
    );
    let response = reqwest::get(&url).await?;

    if !response.status().is_success() {
        return Ok(format!("Error: HTTP {}", response.status()));
    }

    let data: AbilityData = response.json().await?;

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

    Ok(format!(
        "Ability: {}\nEffect: {}\nPokémon with this ability: {}",
        name,
        effect,
        pokemon_list.join(", ")
    ))
}

/// Fetch type effectiveness (weaknesses, resistances, immunities).
///
/// * type_name - The name of the type in lowercase.
#[ollama_rs::function]
pub async fn fetch_type_effectiveness(type_name: String) -> ToolResult<String> {

    let url = format!(
        "https://pokeapi.co/api/v2/type/{}/",
        type_name.to_lowercase()
    );
    let response = reqwest::get(&url).await?;

    if !response.status().is_success() {
        return Ok(format!("Error: HTTP {}", response.status()));
    }

    let data: TypeData = response.json().await?;
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

    Ok(format!(
        "{} Type:\n\nWeak to (2x damage): {}\nResistant to (0.5x damage): {}\nImmune to (0x damage): {}\n\nSuper effective against (2x): {}\nNot very effective against (0.5x): {}",
        capitalize(&type_name),
        format_list(&double_damage_from),
        format_list(&half_damage_from),
        format_list(&no_damage_from),
        format_list(&double_damage_to),
        format_list(&half_damage_to)
    ))
}

/// Fetch detailed information about a Pokémon move.
///
/// * move_name - The name of the move in lowercase.
#[ollama_rs::function]
pub async fn fetch_move_details(move_name: String) -> ToolResult<String> {

    let url = format!(
        "https://pokeapi.co/api/v2/move/{}/",
        move_name.to_lowercase()
    );
    let response = reqwest::get(&url).await?;

    if !response.status().is_success() {
        return Ok(format!("Error: HTTP {}", response.status()));
    }

    let data: MoveData = response.json().await?;

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

    Ok(format!(
        "Move: {}\nType: {}\nCategory: {}\nPower: {}\nAccuracy: {}\nPP: {}\nPriority: {}\nEffect: {}",
        name,
        capitalize(&data.type_info.name),
        capitalize(&data.damage_class.name),
        power,
        accuracy,
        data.pp,
        data.priority,
        effect
    ))
}

/// Fetch comprehensive information about a Pokémon.
/// This combines basic info, stats, and abilities into one response.
///
/// * pokemon_name - The name of the Pokémon in lowercase.
#[ollama_rs::function]
pub async fn fetch_pokemon(pokemon_name: String) -> ToolResult<String> {

    let url = format!(
        "https://pokeapi.co/api/v2/pokemon/{}/",
        pokemon_name.to_lowercase()
    );
    let response = reqwest::get(&url).await?;

    if !response.status().is_success() {
        return Ok(format!("Error: HTTP {}", response.status()));
    }

    let data: PokemonData = response.json().await?;
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

    Ok(format!(
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
    ))
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
    }
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
