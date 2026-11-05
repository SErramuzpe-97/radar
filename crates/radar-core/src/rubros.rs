//! Taxonomía comercial detallada compartida por las fuentes y la API.

use serde_json::{json, Value};

pub const FILTROS: &[(&str, &str)] = &[
    ("bodega", "Bodegas y vino"),
    ("restaurante", "Restaurantes y cartas"),
    ("alojamiento", "Alojamientos turísticos"),
    ("automocion", "Automoción y talleres"),
    ("tanatorio", "Tanatorios y funerarias"),
    ("floristeria", "Floristerías"),
    ("granja_escuela", "Granjas escuela"),
    ("guarderia", "Guarderías"),
    ("veterinaria", "Veterinarias"),
    ("cuidado_mascotas", "Cuidado de mascotas"),
    ("fontaneria", "Fontanería"),
    ("electricidad", "Electricidad"),
    ("cerrajeria", "Cerrajería"),
    ("desatascos", "Desatascos"),
    ("control_plagas", "Control de plagas"),
    ("turismo_cultural", "Turismo y cultura"),
    ("asociacion_ong", "Asociaciones y ONGs"),
    ("otro", "Otros rubros"),
];

pub fn desde_categoria(categoria: Option<&str>, sector: Option<&str>) -> Option<&'static str> {
    let Some(categoria) = categoria else {
        return match sector {
            Some("hosteleria") => Some("restaurante"),
            Some(_) => Some("otro"),
            None => None,
        };
    };
    let c = categoria.rsplit('/').next()?.trim().to_ascii_lowercase();
    let normalized = crate::eligibility::normalize(&c);
    if [
        "bodega",
        "vinicola",
        "viticultura",
        "enoturismo",
        "vinery",
        "elaboraciondevino",
        "producciondevino",
        "vinificacion",
    ]
    .iter()
    .any(|t| normalized.contains(t))
    {
        return Some("bodega");
    }
    if ["funeraria", "tanatorio", "funeral"]
        .iter()
        .any(|t| normalized.contains(t))
    {
        return Some("tanatorio");
    }
    if ["automocion", "tallerdecoches", "carrepair", "autorepair"]
        .iter()
        .any(|t| normalized.contains(t))
    {
        return Some("automocion");
    }
    if ["floristeria", "florist"]
        .iter()
        .any(|t| normalized.contains(t))
    {
        return Some("floristeria");
    }
    if ["granjaescuela", "educacionrural", "farm school"]
        .iter()
        .any(|t| normalized.contains(&t.replace(' ', "")))
    {
        return Some("granja_escuela");
    }
    if [
        "guarderia",
        "escuelainfantil",
        "kindergarten",
        "preschool",
        "nursery",
        "childcare",
        "daycare",
    ]
    .iter()
    .any(|t| normalized.contains(t))
    {
        return Some("guarderia");
    }
    if ["veterinaria", "veterinary"]
        .iter()
        .any(|t| normalized.contains(t))
    {
        return Some("veterinaria");
    }
    if [
        "mascota",
        "petgrooming",
        "petstore",
        "petcare",
        "petsitter",
        "petsitting",
        "animalboarding",
        "dogwalk",
        "cuidadoresdeperros",
    ]
    .iter()
    .any(|t| normalized.contains(t))
    {
        return Some("cuidado_mascotas");
    }
    if ["fontaneria", "plumber", "plumbing"]
        .iter()
        .any(|t| normalized.contains(t))
    {
        return Some("fontaneria");
    }
    if ["electricidad", "electricista", "electrician"]
        .iter()
        .any(|t| normalized.contains(t))
    {
        return Some("electricidad");
    }
    if ["cerrajeria", "locksmith"]
        .iter()
        .any(|t| normalized.contains(t))
    {
        return Some("cerrajeria");
    }
    if ["desatasco", "drenaje", "drainclean", "sewercleaning"]
        .iter()
        .any(|t| normalized.contains(t))
    {
        return Some("desatascos");
    }
    if ["plagas", "pestcontrol", "exterminador"]
        .iter()
        .any(|t| normalized.contains(t))
    {
        return Some("control_plagas");
    }
    if [
        "turismo",
        "museo",
        "atraccion",
        "heritage",
        "cultural",
        "visitorcentre",
    ]
    .iter()
    .any(|t| normalized.contains(t))
    {
        return Some("turismo_cultural");
    }
    Some(match c.as_str() {
        "winery" | "wine" | "bodega" | "vineyard" => "bodega",
        "restaurant"
        | "cafe"
        | "bar"
        | "pub"
        | "fast_food"
        | "bakery"
        | "foodestablishment"
        | "restaurantorcoffeeshop" => "restaurante",
        "car_repair" | "autorepair" | "automotivebusiness" => "automocion",
        "funeral_directors" | "funeral_hall" | "funeralhome" | "mortuary" => "tanatorio",
        "florist" => "floristeria",
        "kindergarten" | "preschool" | "childcare" | "daycare" => "guarderia",
        "veterinary" | "veterinarycare" => "veterinaria",
        "pet_grooming" | "pet_store" | "petstore" | "pet" | "animal_boarding" | "dog_walking"
        | "dog_daycare" => "cuidado_mascotas",
        "plumber" | "plumbing" => "fontaneria",
        "electrician" | "electrical" => "electricidad",
        "locksmith" => "cerrajeria",
        "drain" | "drainage" | "sewer_cleaning" => "desatascos",
        "pest_control" | "exterminator" => "control_plagas",
        "tourist_attraction"
        | "museum"
        | "gallery"
        | "attraction"
        | "zoo"
        | "theme_park"
        | "visitor_centre"
        | "heritage"
        | "castle"
        | "monument"
        | "archaeological_site"
        | "ruins"
        | "petting_zoo" => "turismo_cultural",
        _ => {
            return match sector {
                Some("hosteleria") => Some("restaurante"),
                Some(_) => Some("otro"),
                _ => None,
            }
        }
    })
}

/// Preserva señales de la fuente; no convierte ausencia de tags en ausencia real de servicio.
pub fn evidencia_visita(
    turismo: Option<&str>,
    tours: Option<&str>,
    fee: Option<&str>,
    charge: Option<&str>,
    opening_hours: Option<&str>,
    web_registrada: bool,
) -> Option<Value> {
    let categoria_visitante = turismo.is_some_and(|t| {
        matches!(
            t,
            "attraction"
                | "museum"
                | "gallery"
                | "zoo"
                | "theme_park"
                | "visitor_centre"
                | "castle"
                | "monument"
                | "archaeological_site"
                | "ruins"
                | "petting_zoo"
        )
    });
    let visita = if tours.is_some_and(|t| t.eq_ignore_ascii_case("yes")) {
        "visitas_guiadas_indicadas"
    } else if categoria_visitante {
        "lugar_de_visita_segun_categoria"
    } else {
        return None;
    };
    let tarifa = if fee.is_some_and(|f| f.eq_ignore_ascii_case("yes")) {
        json!({"estado":"declarada_de_pago","fuente":"fee","confianza":"media"})
    } else if fee.is_some_and(|f| f.eq_ignore_ascii_case("no")) {
        json!({"estado":"declarada_gratuita","fuente":"fee"})
    } else if let Some(precio) = charge.filter(|p| !p.trim().is_empty()) {
        if ["0", "free", "gratis", "gratuito"]
            .contains(&precio.trim().to_ascii_lowercase().as_str())
        {
            json!({"estado":"declarada_gratuita","valor":precio,"fuente":"charge","confianza":"media"})
        } else {
            json!({"estado":"precio_declarado","valor":precio,"fuente":"charge","confianza":"media"})
        }
    } else {
        json!({"estado":"sin_dato","confianza":"sin_evidencia"})
    };
    let confianza_visita = if visita == "visitas_guiadas_indicadas" {
        "alta"
    } else {
        "media"
    };
    Some(json!({
        "visitas":{"estado":visita,"fuente":"OpenStreetMap","confianza":confianza_visita,"horario_registrado":opening_hours.is_some()},
        "tarifa":tarifa,
        "presencia_web":{"estado":if web_registrada {"registrada_en_osm"} else {"no_registrada_en_osm"},"fuente":"OpenStreetMap","confianza":"baja"}
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clasifica_rubros_de_osm_y_schema_org() {
        for (entrada, esperado) in [
            ("winery", "bodega"),
            ("shop=florist", "floristeria"),
            ("amenity=kindergarten", "guarderia"),
            ("veterinarycare", "veterinaria"),
            ("craft=plumber", "fontaneria"),
            ("amenity=funeral_hall", "tanatorio"),
            ("tourist_attraction", "turismo_cultural"),
        ] {
            let categoria = entrada.rsplit('=').next().unwrap();
            assert_eq!(
                desde_categoria(Some(categoria), None),
                Some(esperado),
                "{entrada}"
            );
        }
    }

    #[test]
    fn no_infiere_tarifa_a_partir_de_una_categoria_turistica() {
        let e = evidencia_visita(Some("museum"), None, None, None, None, false).unwrap();
        assert_eq!(e["tarifa"]["estado"], "sin_dato");
        assert_eq!(e["presencia_web"]["estado"], "no_registrada_en_osm");
        let e = evidencia_visita(Some("museum"), None, Some("yes"), None, None, true).unwrap();
        assert_eq!(e["tarifa"]["estado"], "declarada_de_pago");
    }
}
