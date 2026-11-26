use crate::model::{BuildCategory, BuildStyle, MaterialPalette, NoiseLevel, RoofStyle};
use crate::noise::SplitMix64;

/// High-level functional theme modulating specialized roofs, facades, and architectural landmarks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StructureTheme {
    Standard,
    Windmill,
    Watermill,
    Lighthouse,
    Greenhouse,
    Tavern,
    Library,
    Colosseum,
    Observatory,
    Pyramid,
    TriumphalArch,
    ClockTower,
    Cathedral,
    Bazaar,
    Treehouse,
    OnsenBath,
    JapaneseCastle,
    // --- Phase 10: Extended architectural monument & regional catalog (44 themes) ---
    Pantheon,
    Parthenon,
    HangingGardens,
    MosquePalace,
    IshtarGate,
    AngkorTemple,
    WaterShrine,
    Sphinx,
    WalledBaileyCastle,
    CityGatehouse,
    MonasteryCloister,
    JoustingArena,
    MilitaryOutpost,
    StarFort,
    Apothecary,
    WineryBakery,
    TownHall,
    OperaHouse,
    Museum,
    BankVault,
    Academy,
    SailingShip,
    ShipyardDock,
    TrainStation,
    AirshipDock,
    DamBarrage,
    CircusTent,
    Skyscraper,
    SteampunkFactory,
    ModernStadium,
    Bunker,
    LaunchpadRocket,
    OffshoreRig,
    MageTower,
    HobbitHole,
    MushroomHouse,
    MausoleumCrypt,
    DwarvenHall,
    DragonSanctuary,
    StoneCirclePortal,
    VikingLonghouse,
    AlpineChalet,
    DesertKasbah,
    FloatingVillage,
}

/// Distinct geometric archetypes for bridges and viaducts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BridgeVariant {
    /// Single bold parabolic/vaulted arch with open spandrels and central overlook balcony.
    GrandSingleArch,
    /// Multi-span semicircular Roman/medieval masonry arches with cutwater piers.
    MasonryArch,
    /// Pointed Gothic lancet arches with ribbed vault soffits, flying buttresses & tall lamp spires.
    GothicRibbedArch,
    /// Two-tier classical Roman aqueduct with top-deck water channel over tall double arcades.
    Aqueduct,
    /// Monumental suspension bridge with tall H-pylons and catenary cables.
    Suspension,
    /// Timber-truss covered bridge with gabled/hipped wooden canopy roof.
    CoveredTimber,
    /// High-arching Japanese Taiko crescent moon bridge with lakeside Torii portals.
    TaikoMoon,
    /// Fortified stone bridge with battlemented gatehouse towers and arched portcullis openings.
    FortifiedGate,
    /// Low timber boardwalk pier with pilings, docked barge moorings & mid-span gazebo.
    PontoonPier,
}

/// Architectural constraints extracted from category, style, bounds, description text, and seed.
#[derive(Clone, Debug, PartialEq)]
pub struct BlueprintSpec {
    pub category: BuildCategory,
    pub style: BuildStyle,
    pub theme: StructureTheme,
    pub bridge_variant: BridgeVariant,
    pub floors: u16,
    pub floor_height: u16,
    pub roof_style: RoofStyle,
    pub roof_overhang: bool,
    pub dormers: bool,
    pub overhang_jetty: bool,
    pub basement: bool,
    pub exterior_depth: bool,
    pub porch: bool,
    pub balcony: bool,
    pub chimney: bool,
    pub corner_turret: bool,
    pub shutters: bool,
    pub flower_boxes: bool,
    pub overgrown: bool,
    pub furnished: bool,
    pub room_partitions: bool,
    pub courtyard_garden: bool,
    pub courtyard_well: bool,
    pub wing_bias: f32,
    pub window_density: f32,
    pub wide_windows: bool,
    pub weathering: f32,
    pub ruin_erosion: f32,
    pub asymmetry: f32,
    pub foundation_height: u16,
    pub garden_lanterns: bool,
    pub symmetric_modern: bool,
    pub stepped_wing: bool,
    pub market_awning: bool,
    pub arcade_gallery: bool,
    pub half_hip: bool,
    pub rose_window: bool,
    pub on_stilts: bool,
    pub ground_wall: Option<String>,
    pub upper_wall: Option<String>,
}

fn parse_material_from_clause(clause: &str) -> Option<String> {
    let lower = clause.to_lowercase();
    if contains_any(&lower, &["булыж", "cobble"]) {
        return Some(if contains_any(&lower, &["мхом", "мшист", "mossy"]) {
            "minecraft:mossy_cobblestone".into()
        } else {
            "minecraft:cobblestone".into()
        });
    }
    if contains_any(&lower, &["сланц", "сланец", "deepslate", "slate"]) {
        return Some("minecraft:deepslate_bricks".into());
    }
    if contains_any(&lower, &["чернит", "чернокам", "blackstone"]) {
        return Some("minecraft:polished_blackstone_bricks".into());
    }
    if contains_any(&lower, &["туф", "tuff"]) {
        return Some("minecraft:tuff_bricks".into());
    }
    if contains_any(&lower, &["кирпич", "brick"]) {
        return Some("minecraft:bricks".into());
    }
    if contains_any(&lower, &["кварц", "мрамор", "quartz", "marble"]) {
        return Some("minecraft:smooth_quartz".into());
    }
    if contains_any(&lower, &["песчан", "sandstone"]) {
        return Some(if contains_any(&lower, &["красн", "red"]) {
            "minecraft:red_sandstone".into()
        } else {
            "minecraft:smooth_sandstone".into()
        });
    }
    if contains_any(&lower, &["бетон", "concrete"]) {
        return Some(if contains_any(&lower, &["сер", "gray"]) {
            "minecraft:gray_concrete".into()
        } else if contains_any(&lower, &["черн", "чёрн", "black"]) {
            "minecraft:black_concrete".into()
        } else {
            "minecraft:white_concrete".into()
        });
    }
    if has_pair(&lower, &["бледн"], &["дуб", "дерев", "стен", "этаж"]) || contains_any(&lower, &["pale oak"]) {
        return Some("minecraft:pale_oak_planks".into());
    }
    if has_pair(&lower, &["темн", "тёмн", "черн", "чёрн"], &["дуб"]) || contains_any(&lower, &["dark oak"]) {
        return Some("minecraft:dark_oak_planks".into());
    }
    if contains_any(&lower, &["берёз", "берез", "birch"]) {
        return Some("minecraft:birch_planks".into());
    }
    if contains_any(&lower, &["ел", "хвой", "spruce"]) {
        return Some("minecraft:spruce_planks".into());
    }
    if contains_any(&lower, &["вишн", "сакур", "cherry", "sakura"]) {
        return Some("minecraft:cherry_planks".into());
    }
    if contains_any(&lower, &["акац", "acacia"]) {
        return Some("minecraft:acacia_planks".into());
    }
    if contains_any(&lower, &["бамбук", "bamboo"]) {
        return Some("minecraft:bamboo_planks".into());
    }
    if contains_any(&lower, &["дуб", "дерев", "oak", "wood", "timber"]) {
        return Some("minecraft:oak_planks".into());
    }
    if contains_any(&lower, &["камен", "из камн", "stone"]) {
        return Some("minecraft:stone_bricks".into());
    }
    None
}

fn extract_floor_wall_overrides(lower: &str) -> (Option<String>, Option<String>) {
    let lower_markers = [
        "1 этаж",
        "1-этаж",
        "1-й этаж",
        "первый этаж",
        "первом этаже",
        "низ ",
        "нижн",
        "ground floor",
        "first floor",
    ];
    let upper_markers = [
        "2 этаж",
        "2-этаж",
        "2-й этаж",
        "второй этаж",
        "втором этаже",
        "верхн",
        "верх ",
        "upper floor",
        "second floor",
    ];

    let mut ground_wall = None;
    let mut upper_wall = None;
    for raw_clause in lower.split([',', ';', '.']) {
        let c = raw_clause.trim();
        if c.is_empty() {
            continue;
        }
        let has_lower = lower_markers.iter().any(|m| c.contains(m));
        let has_upper = upper_markers.iter().any(|m| c.contains(m));
        if has_lower && !has_upper && ground_wall.is_none() {
            ground_wall = parse_material_from_clause(c);
        } else if has_upper && !has_lower && upper_wall.is_none() {
            upper_wall = parse_material_from_clause(c);
        } else if has_lower && has_upper {
            if let Some(pos) = upper_markers.iter().filter_map(|m| c.find(m)).min() {
                if ground_wall.is_none() {
                    ground_wall = parse_material_from_clause(&c[..pos]);
                }
                if upper_wall.is_none() {
                    upper_wall = parse_material_from_clause(&c[pos..]);
                }
            }
        }
    }
    (ground_wall, upper_wall)
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

/// Detects specialized structural themes (mills, monuments, castles, transport, fantasy, regional, etc.).
pub fn infer_theme_from_text(description: &str) -> StructureTheme {
    let lower = description.to_lowercase();
    if contains_any(&lower, &["водян мельниц", "водяная мельниц", "мельница с колес", "watermill", "water mill", "waterwheel"]) {
        StructureTheme::Watermill
    } else if contains_any(&lower, &["мельниц", "ветрян", "windmill"]) {
        StructureTheme::Windmill
    } else if contains_any(&lower, &["маяк", "lighthouse"]) {
        StructureTheme::Lighthouse
    } else if contains_any(&lower, &["пантеон", "pantheon"]) {
        StructureTheme::Pantheon
    } else if contains_any(&lower, &["парфенон", "периптер", "акрополь", "parthenon", "peripter", "acropolis"]) {
        StructureTheme::Parthenon
    } else if contains_any(&lower, &["висячие сад", "семирамид", "hanging garden"]) {
        StructureTheme::HangingGardens
    } else if contains_any(&lower, &["мечет", "минарет", "султан двор", "айван", "mosque", "minaret"]) {
        StructureTheme::MosquePalace
    } else if contains_any(&lower, &["иштар", "вавилон", "лазур ворот", "ishtar", "babylon"]) {
        StructureTheme::IshtarGate
    } else if contains_any(&lower, &["ангкор", "кхмер", "ступа", "пранг", "angkor", "stupa"]) {
        StructureTheme::AngkorTemple
    } else if contains_any(&lower, &["тории алл", "аллея тори", "святилищ на вод", "ицукусим", "фусими", "water shrine", "itsukushima", "fushimi"]) {
        StructureTheme::WaterShrine
    } else if contains_any(&lower, &["сфинкс", "sphinx"]) {
        StructureTheme::Sphinx
    } else if contains_any(&lower, &["бейли", "замок с двор", "замок со стен", "концентрическ замок", "донжон и стен", "bailey", "walled castle", "motte"]) {
        StructureTheme::WalledBaileyCastle
    } else if contains_any(&lower, &["городск ворот", "крепостн ворот", "врата город", "барбакан", "city gate", "barbican"]) {
        StructureTheme::CityGatehouse
    } else if contains_any(&lower, &["монастыр", "аббатств", "клуаτρ", "клуатр", "monastery", "abbey", "cloister"]) {
        StructureTheme::MonasteryCloister
    } else if contains_any(&lower, &["ристалищ", "турнир", "joust", "tournament"]) {
        StructureTheme::JoustingArena
    } else if contains_any(&lower, &["военн лагер", "осадн лагер", "форпост", "частокол", "требушет", "outpost", "military camp", "palisade"]) {
        StructureTheme::MilitaryOutpost
    } else if contains_any(&lower, &["звёздчат форт", "звездчат форт", "вобан", "бастионн форт", "star fort", "bastion fort", "vauban"]) {
        StructureTheme::StarFort
    } else if contains_any(&lower, &["аптек", "лечебниц", "зельевар", "алхим лавк", "госпитал", "больниц", "apothecary", "hospital", "alchemy shop"]) {
        StructureTheme::Apothecary
    } else if contains_any(&lower, &["винодельн", "пивоварн", "пекарн", "булочн", "винный погреб", "winery", "brewery", "bakery"]) {
        StructureTheme::WineryBakery
    } else if contains_any(&lower, &["ратуш", "мэри", "муниципал", "town hall", "city hall"]) {
        StructureTheme::TownHall
    } else if contains_any(&lower, &["оперн", "опер", "театр", "сцена зал", "opera", "theater", "theatre"]) {
        StructureTheme::OperaHouse
    } else if contains_any(&lower, &["музе", "выставк", "экспозиц", "скелет динозавр", "museum"]) {
        StructureTheme::Museum
    } else if contains_any(&lower, &["банк", "сокровищниц", "сейф", "хранилищ золот", "bank", "vault", "treasury"]) {
        StructureTheme::BankVault
    } else if contains_any(&lower, &["университет", "академи", "хогвартс", "большой зал", "колледж", "university", "academy", "great hall"]) {
        StructureTheme::Academy
    } else if contains_any(&lower, &["верф", "морск порт", "гаван", "стапел", "shipyard", "harbor", "harbour", "seaport"]) {
        StructureTheme::ShipyardDock
    } else if contains_any(&lower, &["дирижабл", "небесн причал", "цеппелин", "airship", "zeppelin", "sky dock"]) {
        StructureTheme::AirshipDock
    } else if contains_any(&lower, &["пиратск кораб", "корабл", "галеон", "фрегат", "драккар", "джонк", "ship", "galleon", "frigate", "pirate ship"]) {
        StructureTheme::SailingShip
    } else if contains_any(&lower, &["вокзал", "железнодорож", "станц поезд", "паровоз", "дебаркадер", "train station", "railway"]) {
        StructureTheme::TrainStation
    } else if contains_any(&lower, &["плотин", "дамб", "гэс", "водосброс", "dam ", "dam,", "barrage", "hydroelectric"]) || lower.trim() == "dam" {
        StructureTheme::DamBarrage
    } else if contains_any(&lower, &["цирк", "шапито", "манеж", "circus", "big top"]) {
        StructureTheme::CircusTent
    } else if contains_any(&lower, &["небоскр", "высотк", "бизнес центр", "skyscraper", "highrise"]) {
        StructureTheme::Skyscraper
    } else if contains_any(&lower, &["завод", "фабрик", "мануфактур", "factory", "industrial plant"]) {
        StructureTheme::SteampunkFactory
    } else if contains_any(&lower, &["стадион", "футбольн", "спорт арен", "stadium", "football"]) {
        StructureTheme::ModernStadium
    } else if contains_any(&lower, &["бункер", "убежищ", "капонир", "bunker", "shelter"]) {
        StructureTheme::Bunker
    } else if contains_any(&lower, &["космодром", "ракет", "стартов площад", "шаттл", "launchpad", "rocket", "spaceport"]) {
        StructureTheme::LaunchpadRocket
    } else if contains_any(&lower, &["нефтян", "буров", "морск платформ", "oil rig", "offshore"]) {
        StructureTheme::OffshoreRig
    } else if contains_any(&lower, &["башня мага", "башня архимаг", "магическ башн", "волшебн башн", "башня волшеб", "wizard tower", "mage tower"]) {
        StructureTheme::MageTower
    } else if contains_any(&lower, &["хоббит", "нор в холм", "шир ", "шир,", "hobbit", "shire"]) || lower.trim() == "шир" {
        StructureTheme::HobbitHole
    } else if contains_any(&lower, &["грибн", "мухомор", "гриб дом", "дом гриб", "mushroom"]) {
        StructureTheme::MushroomHouse
    } else if contains_any(&lower, &["склеп", "мавзоле", "крипт", "катакомб", "усыпальниц", "mausoleum", "crypt", "catacombs"]) {
        StructureTheme::MausoleumCrypt
    } else if contains_any(&lower, &["дварф", "гном", "эребор", "мория", "врата в гор", "dwarven", "dwarf", "erebor"]) {
        StructureTheme::DwarvenHall
    } else if contains_any(&lower, &["дракон", "левиафан", "скелет гигант", "dragon", "leviathan", "fossil"]) {
        StructureTheme::DragonSanctuary
    } else if contains_any(&lower, &["стоунхендж", "кромлех", "мегалит", "ритуальн круг", "stonehenge", "cromlech", "stone circle"]) {
        StructureTheme::StoneCirclePortal
    } else if contains_any(&lower, &["длинн дом", "длинный дом", "зал викинг", "медов зал", "мидгард", "longhouse", "viking hall", "mead hall"]) {
        StructureTheme::VikingLonghouse
    } else if contains_any(&lower, &["шале", "альпийск", "горн дом", "chalet", "alpine"]) {
        StructureTheme::AlpineChalet
    } else if contains_any(&lower, &["касб", "риад", "бадгир", "ветроуловит", "мароккан", "kasbah", "riad"]) {
        StructureTheme::DesertKasbah
    } else if contains_any(&lower, &["город на вод", "деревня на вод", "венеци", "свайн город", "floating village", "venice", "canal town"]) {
        StructureTheme::FloatingVillage
    } else if contains_any(&lower, &["обсерватор", "телескоп", "планетар", "observatory", "planetarium", "telescope"]) {
        StructureTheme::Observatory
    } else if contains_any(&lower, &["пирамид", "зиккурат", "зикурат", "майя храм", "pyramid", "ziggurat", "aztec"]) {
        StructureTheme::Pyramid
    } else if contains_any(&lower, &["триумфаль", "триумф арк", "памятн арк", "обелиск", "мемориал", "triumphal", "triumph arch", "obelisk", "monument"]) {
        StructureTheme::TriumphalArch
    } else if contains_any(&lower, &["биг-бен", "биг бен", "часов башн", "часовая башн", "башня с час", "курант", "колокольн", "clocktower", "clock tower", "big ben", "belfry"]) {
        StructureTheme::ClockTower
    } else if contains_any(&lower, &["тенсю", "тэнсю", "химэдзи", "японский замок", "замок сёгун", "замок самур", "japanese castle", "tenshu", "himeji", "shogun castle"]) {
        StructureTheme::JapaneseCastle
    } else if contains_any(&lower, &["кафедрал", "базилик", "собор", "cathedral", "basilica"]) {
        StructureTheme::Cathedral
    } else if contains_any(&lower, &["онсэн", "баня", "терм", "горячий источ", "купальн", "хаммам", "onsen", "bathhouse", "thermae", "hot spring"]) {
        StructureTheme::OnsenBath
    } else if contains_any(&lower, &["дом на дерев", "домик на дерев", "хижина на дерев", "крона дерев", "эвоки", "эльфийский дом на", "treehouse", "tree house"]) {
        StructureTheme::Treehouse
    } else if contains_any(&lower, &["базар", "ярмарк", "торгов ряд", "торгов площад", "восточный рын", "bazaar", "souk", "market square", "fairground"]) {
        StructureTheme::Bazaar
    } else if contains_any(&lower, &["колизе", "амфитеатр", "арен", "гладиатор", "colosseum", "coliseum", "amphitheater", "amphitheatre", "arena"]) {
        StructureTheme::Colosseum
    } else if contains_any(&lower, &["теплиц", "оранжере", "зимн сад", "greenhouse", "conservatory"]) {
        StructureTheme::Greenhouse
    } else if contains_any(&lower, &["таверн", "трактир", "паб", "бар ", "гостиниц", "tavern", "inn", "pub"]) {
        StructureTheme::Tavern
    } else if contains_any(&lower, &["библиотек", "архив", "читальн", "library", "archive"]) {
        StructureTheme::Library
    } else {
        StructureTheme::Standard
    }
}

/// Resolves a rich bridge archetype from description text, architectural style, porch flag, and seed.
pub fn infer_bridge_variant(
    description_lower: &str,
    style: BuildStyle,
    porch: bool,
    seed: u64,
) -> BridgeVariant {
    let mut rng = SplitMix64::new(seed ^ 0xB41D_6E5E_ED00_0777);
    if contains_any(description_lower, &["акведук", "водовод", "канал мост", "aqueduct"]) {
        return BridgeVariant::Aqueduct;
    }
    if contains_any(
        description_lower,
        &["вантов", "подвес", "трос", "канат", "цепн", "suspension", "cable", "golden gate"],
    ) {
        return BridgeVariant::Suspension;
    }
    if contains_any(
        description_lower,
        &["крыт", "навес", "кровл", "covered", "chapel bridge"],
    ) || (porch && contains_any(description_lower, &["крыльц", "porch"]))
    {
        return BridgeVariant::CoveredTimber;
    }
    if contains_any(
        description_lower,
        &["лунн", "горбат", "тайко", "тории", "taiko", "moon bridge", "crescent"],
    ) {
        return BridgeVariant::TaikoMoon;
    }
    if contains_any(
        description_lower,
        &["крепост", "замк", "башн", "фортиф", "укреп", "разводн", "fortified", "gatehouse", "tower bridge", "drawbridge"],
    ) {
        return BridgeVariant::FortifiedGate;
    }
    if contains_any(
        description_lower,
        &["понтон", "причал", "пирс", "пристан", "pontoon", "pier", "boardwalk"],
    ) {
        return BridgeVariant::PontoonPier;
    }
    if contains_any(description_lower, &["стрельчат", "остроконеч", "готическ", "lancet", "ribbed"]) {
        return BridgeVariant::GothicRibbedArch;
    }
    if contains_any(description_lower, &["одноароч", "больш арк", "парабол", "single arch", "great arch"]) {
        return BridgeVariant::GrandSingleArch;
    }

    let roll = rng.range_i32(0, 99);
    if contains_any(description_lower, &["ароч", "арк", "виадук", "arch", "viaduct"]) {
        return match style {
            BuildStyle::Japanese => match roll {
                0..=39 => BridgeVariant::TaikoMoon,
                40..=69 => BridgeVariant::GrandSingleArch,
                _ => BridgeVariant::MasonryArch,
            },
            BuildStyle::Gothic => match roll {
                0..=27 => BridgeVariant::GothicRibbedArch,
                28..=54 => BridgeVariant::GrandSingleArch,
                55..=77 => BridgeVariant::MasonryArch,
                _ => BridgeVariant::FortifiedGate,
            },
            BuildStyle::Classical | BuildStyle::Desert => match roll {
                0..=34 => BridgeVariant::MasonryArch,
                35..=64 => BridgeVariant::Aqueduct,
                65..=84 => BridgeVariant::GrandSingleArch,
                _ => BridgeVariant::GothicRibbedArch,
            },
            _ => match roll % 5 {
                0 => BridgeVariant::GrandSingleArch,
                1 => BridgeVariant::MasonryArch,
                2 => BridgeVariant::GothicRibbedArch,
                3 => BridgeVariant::Aqueduct,
                _ => BridgeVariant::TaikoMoon,
            },
        };
    }

    match style {
        BuildStyle::Japanese => match roll % 5 {
            0 => BridgeVariant::TaikoMoon,
            1 => BridgeVariant::CoveredTimber,
            2 => BridgeVariant::GrandSingleArch,
            3 => BridgeVariant::PontoonPier,
            _ => BridgeVariant::MasonryArch,
        },
        BuildStyle::Modern => match roll % 4 {
            0 => BridgeVariant::Suspension,
            1 => BridgeVariant::GrandSingleArch,
            2 => BridgeVariant::Aqueduct,
            _ => BridgeVariant::PontoonPier,
        },
        BuildStyle::Steampunk => match roll % 4 {
            0 => BridgeVariant::Suspension,
            1 => BridgeVariant::CoveredTimber,
            2 => BridgeVariant::Aqueduct,
            _ => BridgeVariant::FortifiedGate,
        },
        BuildStyle::Classical | BuildStyle::Desert => match roll % 4 {
            0 => BridgeVariant::MasonryArch,
            1 => BridgeVariant::Aqueduct,
            2 => BridgeVariant::GrandSingleArch,
            _ => BridgeVariant::FortifiedGate,
        },
        BuildStyle::Cottage | BuildStyle::Nordic => match roll % 5 {
            0 => BridgeVariant::CoveredTimber,
            1 => BridgeVariant::GrandSingleArch,
            2 => BridgeVariant::MasonryArch,
            3 => BridgeVariant::PontoonPier,
            _ => BridgeVariant::Suspension,
        },
        _ => match roll % 7 {
            0 => BridgeVariant::MasonryArch,
            1 => BridgeVariant::GrandSingleArch,
            2 => BridgeVariant::GothicRibbedArch,
            3 => BridgeVariant::FortifiedGate,
            4 => BridgeVariant::CoveredTimber,
            5 => BridgeVariant::Suspension,
            _ => BridgeVariant::Aqueduct,
        },
    }
}

/// Resolves an explicit or text-inferred architectural style; falls back to seed when Auto.
pub fn infer_style_from_text(requested: BuildStyle, description: &str, seed: u64) -> BuildStyle {
    if requested != BuildStyle::Auto {
        return requested;
    }

    let lower = description.to_lowercase();
    if contains_any(&lower, &["стимпанк", "завод", "фабрик", "паро", "механизм", "часов башн", "часовая башн", "биг-бен", "биг бен", "дирижабл", "вокзал", "steampunk", "industrial", "factory", "airship"]) {
        return BuildStyle::Steampunk;
    }
    if contains_any(&lower, &["античн", "римск", "греческ", "колизе", "амфитеатр", "парфенон", "пантеон", "акрополь", "триумф", "обелиск", "терм", "акведук", "ратуш", "музе", "опер", "classical", "roman", "greek", "antique", "colosseum", "coliseum", "amphitheater", "parthenon", "pantheon", "aqueduct"]) {
        return BuildStyle::Classical;
    }
    if contains_any(&lower, &["япон", "пагод", "самура", "сакур", "бамбук", "энгава", "тоори", "тории", "онсэн", "тенсю", "тэнсю", "химэдзи", "тайко", "ицукусим", "фусими", "japanese", "pagoda", "oriental", "zen", "onsen", "tenshu", "himeji", "taiko"]) {
        return BuildStyle::Japanese;
    }
    if contains_any(&lower, &["готич", "кафедрал", "собор", "вампир", "некро", "мрач", "темн", "тёмн", "сланец", "сланц", "склеп", "мавзоле", "катакомб", "башня мага", "архимаг", "дракон", "gothic", "cathedral", "dark", "crypt", "mausoleum", "mage tower", "wizard tower"]) {
        return BuildStyle::Gothic;
    }
    if contains_any(&lower, &["модерн", "современ", "хайтек", "минимал", "бетон", "кварц", "вилла", "обсерватор", "небоскр", "стадион", "бункер", "космодром", "ракет", "буров", "нефтян", "плотин", "modern", "minimal", "villa", "concrete", "observatory", "skyscraper", "stadium", "bunker", "rocket", "oil rig"]) {
        return BuildStyle::Modern;
    }
    if contains_any(&lower, &["викин", "сканд", "север", "сруб", "длинн", "шале", "альпийск", "nordic", "viking", "longhouse", "chalet", "alpine"]) {
        return BuildStyle::Nordic;
    }
    if contains_any(&lower, &["пустын", "егип", "оазис", "песчан", "араб", "марок", "пирамид", "зиккурат", "базар", "хаммам", "сфинкс", "иштар", "мечет", "минарет", "касб", "риад", "висячие сад", "desert", "arabian", "oasis", "sandstone", "pyramid", "ziggurat", "bazaar", "mosque", "sphinx", "kasbah"]) {
        return BuildStyle::Desert;
    }
    if contains_any(&lower, &["средневек", "тюдор", "фахверк", "рыцар", "бейли", "ристалищ", "барбакан", "монастыр", "аббатств", "верф", "корабл", "галеон", "таверн", "дварф", "medieval", "tudor", "monastery", "shipyard", "galleon", "joust"]) {
        return BuildStyle::Medieval;
    }
    if contains_any(&lower, &["коттедж", "лесн", "сказоч", "сад", "цвет", "плющ", "уютн", "хоббит", "гриб", "мухомор", "винодельн", "пекарн", "аптек", "cottage", "rustic", "cozy", "garden", "hobbit", "mushroom", "bakery"]) {
        return BuildStyle::Cottage;
    }

    let mut rng = SplitMix64::new(seed ^ 0x4567_89AB_CDEF_0123);
    match rng.range_i32(0, 8) {
        0 => BuildStyle::Medieval,
        1 => BuildStyle::Cottage,
        2 => BuildStyle::Japanese,
        3 => BuildStyle::Gothic,
        4 => BuildStyle::Nordic,
        5 => BuildStyle::Modern,
        6 => BuildStyle::Desert,
        7 => BuildStyle::Steampunk,
        _ => BuildStyle::Classical,
    }
}

/// Parses a Russian or English building description into structured noise + grammar parameters.
pub fn parse_description(
    category: BuildCategory,
    requested_style: BuildStyle,
    description: &str,
    width: u16,
    depth: u16,
    height: u16,
    noise_level: NoiseLevel,
    seed: u64,
) -> BlueprintSpec {
    let lower = description.to_lowercase();
    let mut rng = SplitMix64::new(seed ^ 0x1357_2468_9ABC_DEF0);

    let base_asym = noise_level.strength();
    let theme = infer_theme_from_text(&lower);

    let effective_cat = if matches!(theme, StructureTheme::ShipyardDock | StructureTheme::FloatingVillage)
        || contains_any(&lower, &["квартал", "деревн", "поселен", "посёлок", "поселок", "усадьб", "городок", "village", "town", "hamlet", "settlement", "district", "compound"])
    {
        BuildCategory::Settlement
    } else if theme == StructureTheme::DamBarrage
        || contains_any(&lower, &["мост", "виадук", "переправ", "акведук", "bridge", "viaduct", "aqueduct"])
    {
        BuildCategory::Bridge
    } else if matches!(
        theme,
        StructureTheme::Colosseum
            | StructureTheme::JapaneseCastle
            | StructureTheme::IshtarGate
            | StructureTheme::WalledBaileyCastle
            | StructureTheme::CityGatehouse
            | StructureTheme::JoustingArena
            | StructureTheme::MilitaryOutpost
            | StructureTheme::StarFort
            | StructureTheme::ModernStadium
            | StructureTheme::Bunker
            | StructureTheme::DwarvenHall
    ) {
        BuildCategory::Castle
    } else if contains_any(&lower, &["цитадел", "замок", "крепост", "форт", "castle", "citadel", "fortress"]) {
        BuildCategory::Castle
    } else if matches!(theme, StructureTheme::Watermill | StructureTheme::SteampunkFactory)
        || contains_any(&lower, &["кузниц", "горн", "наковал", "мастерск", "forge", "smithy", "workshop"])
    {
        BuildCategory::Forge
    } else if matches!(
        theme,
        StructureTheme::Cathedral
            | StructureTheme::Pyramid
            | StructureTheme::TriumphalArch
            | StructureTheme::Pantheon
            | StructureTheme::Parthenon
            | StructureTheme::MosquePalace
            | StructureTheme::AngkorTemple
            | StructureTheme::WaterShrine
            | StructureTheme::Sphinx
            | StructureTheme::MonasteryCloister
            | StructureTheme::Academy
            | StructureTheme::MausoleumCrypt
            | StructureTheme::DragonSanctuary
            | StructureTheme::StoneCirclePortal
    ) || contains_any(&lower, &["храм", "святил", "собор", "алтар", "церков", "парфенон", "пантеон", "акрополь", "temple", "shrine", "cathedral", "parthenon", "pantheon"]) {
        BuildCategory::Temple
    } else if matches!(
        theme,
        StructureTheme::Windmill
            | StructureTheme::Lighthouse
            | StructureTheme::Observatory
            | StructureTheme::ClockTower
            | StructureTheme::AirshipDock
            | StructureTheme::Skyscraper
            | StructureTheme::LaunchpadRocket
            | StructureTheme::OffshoreRig
            | StructureTheme::MageTower
    ) {
        BuildCategory::Tower
    } else if matches!(
        theme,
        StructureTheme::Greenhouse
            | StructureTheme::Bazaar
            | StructureTheme::OnsenBath
            | StructureTheme::HangingGardens
            | StructureTheme::SailingShip
            | StructureTheme::CircusTent
    ) {
        BuildCategory::Pavilion
    } else if matches!(
        theme,
        StructureTheme::Treehouse
            | StructureTheme::Apothecary
            | StructureTheme::WineryBakery
            | StructureTheme::TownHall
            | StructureTheme::OperaHouse
            | StructureTheme::Museum
            | StructureTheme::BankVault
            | StructureTheme::TrainStation
            | StructureTheme::HobbitHole
            | StructureTheme::MushroomHouse
            | StructureTheme::VikingLonghouse
            | StructureTheme::AlpineChalet
            | StructureTheme::DesertKasbah
    ) {
        BuildCategory::House
    } else if contains_any(&lower, &["башня", "дозор", "tower", "watchtower"])
        && !contains_any(&lower, &["дом с баш", "с башенк", "house with tower"])
    {
        BuildCategory::Tower
    } else if contains_any(&lower, &["амбар", "сарай", "склад", "ферм", "конюш", "barn", "warehouse", "stable"]) {
        BuildCategory::Barn
    } else if contains_any(&lower, &["павильон", "беседк", "ротонд", "pavilion", "gazebo"]) {
        BuildCategory::Pavilion
    } else {
        category
    };

    let effective_style = infer_style_from_text(requested_style, &lower, seed);
    let theme = if theme == StructureTheme::Standard
        && effective_cat == BuildCategory::Castle
        && effective_style == BuildStyle::Japanese
    {
        StructureTheme::JapaneseCastle
    } else {
        theme
    };

    let explicit_floors = if contains_any(&lower, &["четырехэтаж", "четырёхэтаж", "4 этаж", "4-этаж", "4 story"]) {
        Some(4)
    } else if contains_any(&lower, &["трехэтаж", "трёхэтаж", "3 этаж", "3-этаж", "3 story"]) {
        Some(3)
    } else if contains_any(&lower, &["двухэтаж", "2 этаж", "2-этаж", "два этажа", "2 story", "two story", "мансард"]) {
        Some(2)
    } else if contains_any(&lower, &["одноэтаж", "1 этаж", "1-этаж", "1 story", "one story", "бунгало"]) {
        Some(1)
    } else {
        None
    };

    let floor_height: u16 = if matches!(effective_cat, BuildCategory::Temple | BuildCategory::Castle)
        || theme == StructureTheme::Library
        || contains_any(&lower, &["высок", "величеств", "собор", "high ceiling", "tall hall", "grand"])
    {
        5
    } else if effective_style == BuildStyle::Gothic || (height >= 14 && explicit_floors.is_none() && rng.next_f64() < 0.35) {
        5
    } else {
        4
    };

    let max_fit_floors = ((height.saturating_sub(3)) / floor_height).max(1);
    let floors = match explicit_floors {
        Some(f) => f.min(max_fit_floors).max(1),
        None => match effective_cat {
            BuildCategory::Tower => {
                let min_f = if matches!(theme, StructureTheme::Windmill | StructureTheme::Lighthouse) {
                    3.min(max_fit_floors)
                } else {
                    2.min(max_fit_floors)
                };
                let span_f = max_fit_floors.min(6).saturating_sub(min_f);
                min_f + if span_f > 0 { rng.range_i32(0, span_f as i32) as u16 } else { 0 }
            }
            BuildCategory::Castle => 2.min(max_fit_floors).max(1),
            BuildCategory::Barn | BuildCategory::Pavilion | BuildCategory::Forge | BuildCategory::Temple | BuildCategory::Bridge | BuildCategory::Settlement => 1,
            BuildCategory::House => {
                let roll = rng.next_f64();
                if theme == StructureTheme::Tavern && max_fit_floors >= 2 {
                    2
                } else if max_fit_floors >= 3 && height >= 16 && roll < 0.25 {
                    3
                } else if max_fit_floors >= 2 && height >= 11 {
                    if roll < 0.65 { 2 } else { 1 }
                } else if max_fit_floors >= 2 && height >= 9 && roll < 0.42 {
                    2
                } else {
                    1
                }
            }
        },
    };

    let roof_style = if contains_any(&lower, &["плоск", "террас на крыш", "бункер", "flat roof", "flat"]) {
        RoofStyle::Flat
    } else if contains_any(&lower, &["купол", "луков", "сферич", "обсерватор", "dome"]) {
        RoofStyle::Dome
    } else if contains_any(&lower, &["мансард", "франц", "особняк", "mansard"]) {
        RoofStyle::Mansard
    } else if contains_any(&lower, &["изогнут", "пагод", "япон", "pagoda", "arched roof"]) {
        RoofStyle::Pagoda
    } else if contains_any(
        &lower,
        &[
            "полувальм",
            "двускат",
            "обычн крыш",
            "half hip",
            "half-hip",
            "dutch",
            "gable",
        ],
    ) {
        RoofStyle::Gable
    } else if contains_any(&lower, &["шатр", "пирам", "шатер", "шатров", "hip", "pyramid"]) {
        RoofStyle::Hip
    } else if contains_any(&lower, &["крут", "остр", "готич", "сканд", "викин", "steep", "gothic", "nordic"]) {
        RoofStyle::Steep
    } else {
        let roll = rng.next_f64();
        match (effective_cat, effective_style, theme) {
            (_, _, StructureTheme::Lighthouse | StructureTheme::CircusTent) => RoofStyle::Hip,
            (_, _, StructureTheme::Observatory | StructureTheme::Pantheon | StructureTheme::MosquePalace) => RoofStyle::Dome,
            (_, _, StructureTheme::ClockTower | StructureTheme::MageTower | StructureTheme::VikingLonghouse) => RoofStyle::Steep,
            (_, _, StructureTheme::Windmill) => RoofStyle::Steep,
            (_, _, StructureTheme::Greenhouse | StructureTheme::Watermill | StructureTheme::Parthenon | StructureTheme::AlpineChalet) => RoofStyle::Gable,
            (_, _, StructureTheme::JapaneseCastle | StructureTheme::OnsenBath | StructureTheme::WaterShrine) => RoofStyle::Pagoda,
            (_, _, StructureTheme::Cathedral | StructureTheme::MausoleumCrypt) => RoofStyle::Steep,
            (_, _, StructureTheme::Pyramid | StructureTheme::TriumphalArch | StructureTheme::Skyscraper | StructureTheme::Bunker | StructureTheme::ModernStadium) => RoofStyle::Flat,
            (BuildCategory::Castle | BuildCategory::Bridge, _, _) => RoofStyle::Flat,
            (_, BuildStyle::Japanese, _) => RoofStyle::Pagoda,
            (_, BuildStyle::Modern, _) => if roll < 0.85 { RoofStyle::Flat } else { RoofStyle::Hip },
            (_, BuildStyle::Desert, _) => {
                if roll < 0.62 {
                    RoofStyle::Flat
                } else if roll < 0.90 {
                    RoofStyle::Dome
                } else {
                    RoofStyle::Hip
                }
            }
            (_, BuildStyle::Gothic | BuildStyle::Nordic, _) => {
                if roll < 0.68 { RoofStyle::Steep } else { RoofStyle::Gable }
            }
            (BuildCategory::Tower, _, _) => {
                if roll < 0.38 {
                    RoofStyle::Steep
                } else if roll < 0.64 {
                    RoofStyle::Hip
                } else if roll < 0.84 {
                    RoofStyle::Dome
                } else {
                    RoofStyle::Flat
                }
            }
            (BuildCategory::Pavilion, _, _) => {
                if roll < 0.50 {
                    RoofStyle::Pagoda
                } else if roll < 0.78 {
                    RoofStyle::Dome
                } else {
                    RoofStyle::Hip
                }
            }
            (BuildCategory::Temple, _, _) => {
                if roll < 0.48 {
                    RoofStyle::Steep
                } else if roll < 0.76 {
                    RoofStyle::Pagoda
                } else {
                    RoofStyle::Dome
                }
            }
            (BuildCategory::Barn | BuildCategory::Forge, _, _) => {
                if roll < 0.65 { RoofStyle::Gable } else { RoofStyle::Steep }
            }
            (BuildCategory::House, BuildStyle::Medieval | BuildStyle::Steampunk, _) => {
                if roll < 0.30 {
                    RoofStyle::Gable
                } else if roll < 0.58 {
                    RoofStyle::Hip
                } else if roll < 0.82 {
                    RoofStyle::Mansard
                } else {
                    RoofStyle::Steep
                }
            }
            _ => {
                if roll < 0.34 {
                    RoofStyle::Gable
                } else if roll < 0.62 {
                    RoofStyle::Hip
                } else if roll < 0.78 {
                    RoofStyle::Steep
                } else if roll < 0.90 {
                    RoofStyle::Mansard
                } else {
                    RoofStyle::Flat
                }
            }
        }
    };

    let symmetric_modern = effective_style == BuildStyle::Modern
        && !contains_any(&lower, &["асимметр", "необыч", "asymmetric", "unusual"])
        || contains_any(&lower, &["симметр", "symmetric"]);
    let ruin_flag = contains_any(&lower, &["руин", "заброш", "разруш", "слом", "ruin", "abandoned", "broken"]);
    let weathered_flag = ruin_flag || contains_any(&lower, &["мхом", "мшист", "зарос", "старин", "стар", "трещин", "mossy", "aged", "old", "weathered"]);
    let courtyard_garden_kw = contains_any(
        &lower,
        &["плющ", "лиан", "листв", "зелен", "сад", "цвет", "пруд", "garden", "pond"],
    );
    let overgrown = theme == StructureTheme::Greenhouse
        || (!contains_any(&lower, &["без сад", "без пруда", "no garden"])
            && (contains_any(
                &lower,
                &["плющ", "лиан", "листв", "зелен", "сад", "цвет", "overgrown", "ivy", "leaf", "garden"],
            ) || (matches!(effective_style, BuildStyle::Cottage | BuildStyle::Japanese) && rng.next_f64() < 0.75)
                || (ruin_flag && rng.next_f64() < 0.75)));

    let porch = if contains_any(&lower, &["без крыльц", "no porch"]) {
        false
    } else {
        theme != StructureTheme::Windmill
            && (matches!(effective_style, BuildStyle::Japanese)
                || theme == StructureTheme::Tavern
                || contains_any(&lower, &["крыльц", "веранд", "террас", "вход", "porch", "veranda", "terrace"])
                || (matches!(effective_cat, BuildCategory::House | BuildCategory::Forge)
                    && effective_style != BuildStyle::Modern
                    && rng.next_f64() < 0.62))
    };

    let balcony = if contains_any(&lower, &["без балкон", "no balcony"]) {
        false
    } else {
        floors >= 2
            && theme != StructureTheme::Windmill
            && (theme == StructureTheme::Lighthouse
                || contains_any(&lower, &["балкон", "террас", "balcony"])
                || (matches!(effective_cat, BuildCategory::House | BuildCategory::Castle) && rng.next_f64() < 0.55))
    };

    let chimney = matches!(effective_cat, BuildCategory::Forge)
        || theme == StructureTheme::Tavern
        || contains_any(&lower, &["камин", "труб", "печ", "дымох", "chimney", "fireplace"])
        || (matches!(effective_cat, BuildCategory::House) && !matches!(effective_style, BuildStyle::Modern | BuildStyle::Desert) && !ruin_flag && rng.next_f64() < 0.65);

    let corner_turret = (width >= 11 && depth >= 11)
        && !matches!(theme, StructureTheme::Windmill | StructureTheme::Lighthouse | StructureTheme::Greenhouse)
        && (matches!(effective_cat, BuildCategory::Castle)
            || contains_any(&lower, &["башен", "башн", "эркер", "шпил", "turret", "tower"])
            || (effective_cat == BuildCategory::House && matches!(effective_style, BuildStyle::Medieval | BuildStyle::Gothic | BuildStyle::Steampunk) && rng.next_f64() < 0.38));

    let wide_windows = effective_style == BuildStyle::Modern
        || matches!(theme, StructureTheme::Greenhouse | StructureTheme::Library | StructureTheme::Lighthouse)
        || contains_any(&lower, &["панорам", "больш окн", "светл", "витраж", "panoram", "large window", "stained glass", "bright"])
        || (!contains_any(&lower, &["бойниц", "мал окн", "без окон", "slit", "small window", "no window"]) && rng.next_f64() < 0.28);
    let small_windows = contains_any(&lower, &["бойниц", "мал окн", "темн", "бункер", "slit", "small window", "dark"])
        || (effective_cat == BuildCategory::Castle && !wide_windows);
    let no_windows = contains_any(&lower, &["без окон", "no window"]);

    let window_density = if no_windows {
        0.0
    } else if wide_windows {
        0.88
    } else if small_windows {
        0.28
    } else if effective_cat == BuildCategory::Barn {
        0.32 + (rng.next_f64() as f32) * 0.22
    } else {
        (0.48 + (rng.next_f64() as f32) * 0.42).clamp(0.30, 0.92)
    };

    let wing_bias = if symmetric_modern && effective_style != BuildStyle::Modern {
        0.20
    } else if contains_any(&lower, &["г-образ", "пристро", "флигел", "сложн", "l-shape", "wing", "asymmetric", "необыч"]) {
        0.95
    } else if effective_style == BuildStyle::Modern {
        0.85
    } else if effective_cat == BuildCategory::House {
        (0.55 + base_asym * 0.35 + (rng.next_f64() as f32) * 0.20).clamp(0.0, 1.0)
    } else {
        base_asym * 0.50
    };

    let basement = if contains_any(&lower, &["без подвал", "no basement"]) {
        false
    } else {
        effective_cat != BuildCategory::Bridge
            && effective_cat != BuildCategory::Pavilion
            && width >= 9
            && depth >= 9
            && (contains_any(&lower, &["подвал", "погреб", "крипт", "подзем", "basement", "cellar", "crypt"])
                || (matches!(effective_cat, BuildCategory::Castle | BuildCategory::Forge) && rng.next_f64() < 0.40))
    };

    let foundation_height = if matches!(effective_cat, BuildCategory::Castle | BuildCategory::Temple)
        || basement
        || contains_any(&lower, &["высок фундамент", "цокол", "сва", "tall foundation"])
    {
        2
    } else if height >= 10 && explicit_floors.is_none() && rng.next_f64() < 0.35 {
        2
    } else {
        1
    };

    let garden_lanterns = contains_any(&lower, &["фонар", "сад", "забор", "двор", "уютн", "lantern", "garden", "cozy"])
        || matches!(theme, StructureTheme::Tavern | StructureTheme::Greenhouse)
        || (matches!(effective_cat, BuildCategory::House | BuildCategory::Temple | BuildCategory::Pavilion) && rng.next_f64() < 0.65);

    let courtyard_garden = if contains_any(&lower, &["без сад", "без пруда", "no garden"]) {
        false
    } else {
        (overgrown || garden_lanterns || courtyard_garden_kw) && width >= 11 && depth >= 10
    };
    let courtyard_well = !matches!(effective_cat, BuildCategory::Bridge | BuildCategory::Pavilion)
        && !ruin_flag
        && width >= 19
        && depth >= 17
        && (contains_any(&lower, &["колод", "двор", "ферм", "сад", "уютн", "well", "courtyard", "garden"])
            || rng.next_f64() < 0.55);

    let stepped_wing = floors >= 2
        && width >= 13
        && depth >= 11
        && !matches!(
            effective_cat,
            BuildCategory::Tower | BuildCategory::Temple | BuildCategory::Bridge | BuildCategory::Pavilion
        )
        && !matches!(
            theme,
            StructureTheme::Windmill | StructureTheme::Lighthouse | StructureTheme::Greenhouse
        )
        && (contains_any(
            &lower,
            &[
                "террас",
                "ярус",
                "асимметр",
                "каскад",
                "флигел",
                "чайн",
                "stepped",
                "tiered",
                "tea",
            ],
        ) || rng.next_f64() < 0.72);

    let market_awning_kw = contains_any(
        &lower,
        &[
            "лавк",
            "рынок",
            "магазин",
            "кафе",
            "маркиз",
            "навес",
            "чайн",
            "прилав",
            "shop",
            "awning",
            "market",
            "stall",
            "cafe",
        ],
    );
    let market_awning = if contains_any(&lower, &["без навес", "без лавк", "no awning"]) {
        false
    } else {
        width >= 11
            && depth >= 9
            && !ruin_flag
            && matches!(
                effective_cat,
                BuildCategory::House | BuildCategory::Forge | BuildCategory::Barn
            )
            && !matches!(
                effective_style,
                BuildStyle::Gothic | BuildStyle::Modern | BuildStyle::Desert
            )
            && (theme == StructureTheme::Tavern
                || market_awning_kw
                || (!courtyard_well && rng.next_f64() < 0.55))
    };

    let arcade_gallery = if contains_any(&lower, &["без галере", "без аркад", "no arcade", "no gallery"]) {
        false
    } else {
        width >= 15
            && depth >= 13
            && !ruin_flag
            && !matches!(effective_cat, BuildCategory::Bridge | BuildCategory::Pavilion)
            && !matches!(
                theme,
                StructureTheme::Greenhouse | StructureTheme::Windmill | StructureTheme::Lighthouse
            )
            && (contains_any(
                &lower,
                &["аркад", "галере", "колоннад", "arcade", "gallery", "cloister"],
            ) || matches!(effective_cat, BuildCategory::Temple | BuildCategory::Castle)
                || (effective_cat == BuildCategory::House
                    && matches!(
                        effective_style,
                        BuildStyle::Medieval | BuildStyle::Cottage | BuildStyle::Desert | BuildStyle::Japanese
                    )
                    && rng.next_f64() < 0.45))
    };

    let half_hip = roof_style == RoofStyle::Gable
        && !matches!(effective_style, BuildStyle::Modern | BuildStyle::Gothic)
        && (contains_any(&lower, &["полувальм", "half hip", "half-hip", "dutch"])
            || (matches!(
                effective_style,
                BuildStyle::Cottage | BuildStyle::Nordic | BuildStyle::Medieval
            ) && width >= 11
                && rng.next_f64() < 0.45));

    let rose_window = !ruin_flag
        && width >= 11
        && (matches!(effective_cat, BuildCategory::Temple | BuildCategory::Castle)
            || effective_style == BuildStyle::Gothic
            || contains_any(
                &lower,
                &["розетк", "роза-витраж", "витраж роза", "аркбутан", "rose window", "buttress"],
            ));

    let is_colosseum = theme == StructureTheme::Colosseum;
    let classical_dome = !is_colosseum
        && contains_any(&lower, &["пантеон", "pantheon"]);
    let resolved_roof = if is_colosseum {
        RoofStyle::Flat
    } else if classical_dome {
        RoofStyle::Dome
    } else if effective_style == BuildStyle::Classical
        && matches!(effective_cat, BuildCategory::Temple | BuildCategory::House)
        && !contains_any(&lower, &["плоск", "купол", "мансард", "пагод", "шат", "flat", "dome", "mansard", "pagoda", "hip"])
    {
        RoofStyle::Gable
    } else {
        roof_style
    };
    let is_classical = effective_style == BuildStyle::Classical;
    let colosseum_basement = is_colosseum
        && contains_any(&lower, &["подвал", "крипт", "подзем", "арен под", "hypogeum", "basement", "crypt", "underground"]);
    let on_stilts = !is_colosseum
        && effective_cat != BuildCategory::Bridge
        && contains_any(
            &lower,
            &[
                "на сваях",
                "сваях",
                "на свая",
                "на воде",
                "пирс",
                "причал",
                "stilt",
                "stilts",
                "on water",
                "boathouse",
                "pier house",
            ],
        );
    let (ground_wall, upper_wall) = if is_colosseum {
        (None, None)
    } else {
        extract_floor_wall_overrides(&lower)
    };

    let bridge_variant = infer_bridge_variant(&lower, effective_style, porch, seed);

    BlueprintSpec {
        category: effective_cat,
        style: effective_style,
        theme,
        bridge_variant,
        floors: if is_colosseum { 1 } else { floors },
        floor_height,
        roof_style: resolved_roof,
        roof_overhang: !is_colosseum && !matches!(resolved_roof, RoofStyle::Flat) && !contains_any(&lower, &["без свеса", "no overhang"]),
        dormers: !is_colosseum
            && !is_classical
            && !matches!(resolved_roof, RoofStyle::Flat)
            && width >= 11
            && depth >= 11
            && !matches!(theme, StructureTheme::Windmill | StructureTheme::Lighthouse)
            && (contains_any(&lower, &["мансард", "слухов", "чердак", "dormer"]) || rng.next_f64() < 0.58),
        overhang_jetty: !is_colosseum
            && !is_classical
            && floors >= 2
            && width >= 11
            && depth >= 11
            && !matches!(theme, StructureTheme::Lighthouse | StructureTheme::Greenhouse)
            && (matches!(effective_style, BuildStyle::Medieval | BuildStyle::Cottage | BuildStyle::Steampunk)
                || theme == StructureTheme::Tavern
                || contains_any(&lower, &["навис", "фахверк", "jetty", "overhang", "tudor"])),
        basement: if is_colosseum { colosseum_basement } else { basement },
        exterior_depth: width >= 11 && depth >= 9 && effective_cat != BuildCategory::Bridge,
        porch: !is_colosseum && porch,
        balcony: !is_colosseum && balcony,
        chimney: !is_colosseum && !is_classical && chimney,
        corner_turret: !is_colosseum && !is_classical && corner_turret,
        shutters: !is_colosseum
            && !matches!(effective_style, BuildStyle::Modern | BuildStyle::Desert | BuildStyle::Classical)
            && theme != StructureTheme::Greenhouse
            && !wide_windows
            && !ruin_flag
            && rng.next_f64() < 0.72,
        flower_boxes: !is_colosseum
            && (matches!(effective_style, BuildStyle::Cottage | BuildStyle::Medieval | BuildStyle::Japanese)
                || overgrown)
            && !ruin_flag
            && rng.next_f64() < 0.68,
        overgrown,
        furnished: !contains_any(&lower, &["пуст", "без мебел", "коробк", "empty", "unfurnished"]),
        room_partitions: !is_colosseum
            && width >= 12
            && depth >= 11
            && matches!(effective_cat, BuildCategory::House | BuildCategory::Castle),
        courtyard_garden: if is_colosseum { courtyard_garden_kw } else { courtyard_garden },
        courtyard_well: !is_colosseum && courtyard_well,
        wing_bias: if is_colosseum { 0.0 } else { wing_bias },
        window_density,
        wide_windows,
        weathering: if ruin_flag {
            0.80
        } else if weathered_flag {
            0.55
        } else {
            (0.24 + base_asym * 0.22 + (rng.next_f64() as f32) * 0.16).clamp(0.15, 0.65)
        },
        ruin_erosion: if ruin_flag { 0.45 } else { 0.0 },
        asymmetry: if symmetric_modern || is_colosseum || is_classical {
            0.0
        } else {
            (base_asym + 0.15).clamp(0.15, 1.0)
        },
        foundation_height: if is_colosseum { 1 } else { foundation_height },
        garden_lanterns: is_colosseum || garden_lanterns,
        symmetric_modern,
        stepped_wing: !is_colosseum && !is_classical && stepped_wing,
        market_awning: !is_colosseum && market_awning,
        arcade_gallery: if is_colosseum {
            false
        } else {
            arcade_gallery
                || (is_classical
                    && width >= 13
                    && depth >= 11
                    && !ruin_flag
                    && !matches!(effective_cat, BuildCategory::Bridge | BuildCategory::Pavilion))
        },
        half_hip: !is_colosseum && !is_classical && half_hip,
        rose_window: !is_colosseum && !is_classical && rose_window,
        on_stilts,
        ground_wall,
        upper_wall,
    }
}

fn has_pair(haystack: &str, a: &[&str], b: &[&str]) -> bool {
    contains_any(haystack, a) && contains_any(haystack, b)
}

fn apply_wood_palette(pal: &mut MaterialPalette, wood: &str) {
    let (planks, log, door, fence, trapdoor, leaves) = match wood {
        "spruce" => (
            "minecraft:spruce_planks",
            "minecraft:spruce_log",
            "minecraft:spruce_door",
            "minecraft:spruce_fence",
            "minecraft:spruce_trapdoor",
            "minecraft:spruce_leaves",
        ),
        "birch" => (
            "minecraft:birch_planks",
            "minecraft:birch_log",
            "minecraft:birch_door",
            "minecraft:birch_fence",
            "minecraft:birch_trapdoor",
            "minecraft:birch_leaves",
        ),
        "cherry" => (
            "minecraft:cherry_planks",
            "minecraft:cherry_log",
            "minecraft:cherry_door",
            "minecraft:cherry_fence",
            "minecraft:cherry_trapdoor",
            "minecraft:cherry_leaves",
        ),
        "pale_oak" => (
            "minecraft:pale_oak_planks",
            "minecraft:pale_oak_log",
            "minecraft:pale_oak_door",
            "minecraft:pale_oak_fence",
            "minecraft:pale_oak_trapdoor",
            "minecraft:pale_oak_leaves",
        ),
        "dark_oak" => (
            "minecraft:dark_oak_planks",
            "minecraft:dark_oak_log",
            "minecraft:dark_oak_door",
            "minecraft:dark_oak_fence",
            "minecraft:dark_oak_trapdoor",
            "minecraft:dark_oak_leaves",
        ),
        "acacia" => (
            "minecraft:acacia_planks",
            "minecraft:acacia_log",
            "minecraft:acacia_door",
            "minecraft:acacia_fence",
            "minecraft:acacia_trapdoor",
            "minecraft:acacia_leaves",
        ),
        "jungle" => (
            "minecraft:jungle_planks",
            "minecraft:jungle_log",
            "minecraft:jungle_door",
            "minecraft:jungle_fence",
            "minecraft:jungle_trapdoor",
            "minecraft:jungle_leaves",
        ),
        "mangrove" => (
            "minecraft:mangrove_planks",
            "minecraft:mangrove_log",
            "minecraft:mangrove_door",
            "minecraft:mangrove_fence",
            "minecraft:mangrove_trapdoor",
            "minecraft:mangrove_leaves",
        ),
        "bamboo" => (
            "minecraft:bamboo_planks",
            "minecraft:bamboo_block",
            "minecraft:bamboo_door",
            "minecraft:bamboo_fence",
            "minecraft:bamboo_trapdoor",
            "minecraft:jungle_leaves",
        ),
        "crimson" => (
            "minecraft:crimson_planks",
            "minecraft:crimson_stem",
            "minecraft:crimson_door",
            "minecraft:crimson_fence",
            "minecraft:crimson_trapdoor",
            "minecraft:flowering_azalea_leaves",
        ),
        "warped" => (
            "minecraft:warped_planks",
            "minecraft:warped_stem",
            "minecraft:warped_door",
            "minecraft:warped_fence",
            "minecraft:warped_trapdoor",
            "minecraft:azalea_leaves",
        ),
        _ => (
            "minecraft:oak_planks",
            "minecraft:oak_log",
            "minecraft:oak_door",
            "minecraft:oak_fence",
            "minecraft:oak_trapdoor",
            "minecraft:oak_leaves",
        ),
    };
    pal.wall = planks.into();
    pal.wall_upper = planks.into();
    pal.wall_accent = if matches!(wood, "crimson" | "warped" | "bamboo") {
        log.into()
    } else {
        format!("minecraft:stripped_{wood}_log")
    };
    pal.wall_trim = pal.wall_accent.clone();
    pal.frame = log.into();
    pal.beam = log.into();
    pal.floor = if wood == "bamboo" {
        "minecraft:bamboo_mosaic".into()
    } else {
        planks.into()
    };
    pal.floor_upper = planks.into();
    pal.ceiling = format!("minecraft:{wood}_slab");
    pal.door = door.into();
    pal.interior_door = door.into();
    pal.fence = fence.into();
    pal.fence_gate = format!("minecraft:{wood}_fence_gate");
    pal.trapdoor = trapdoor.into();
    pal.leaves = leaves.into();
}

/// Suggests a fitting rich material palette synced to the exact architectural seed and text description.
pub fn infer_palette_with_style_and_seed(
    requested_style: BuildStyle,
    description: &str,
    seed: u64,
) -> MaterialPalette {
    let lower = description.to_lowercase();
    let style = infer_style_from_text(requested_style, &lower, seed);
    let theme = infer_theme_from_text(&lower);

    let mut pal = if contains_any(&lower, &["призмарин", "морск", "океан", "подвод", "prismarine", "ocean", "aquatic"]) {
        MaterialPalette::preset(10)
    } else if contains_any(&lower, &["зимн", "снег", "снеж", "лед", "лёд", "мороз", "шале", "winter", "ice", "snow", "frost"]) {
        MaterialPalette::preset(15)
    } else if contains_any(&lower, &["аметист", "магич", "волшеб", "пурпур", "энд", "amethyst", "magic", "purpur", "arcane"]) {
        MaterialPalette::preset(12)
    } else if contains_any(&lower, &["адск", "незер", "преиспод", "nether"]) {
        MaterialPalette::preset(13)
    } else if has_pair(&lower, &["бледн"], &["дуб", "лес", "дом"]) || contains_any(&lower, &["pale oak"]) {
        MaterialPalette::preset(14)
    } else if contains_any(&lower, &["вишн", "сакур", "cherry", "sakura"])
        && !contains_any(&lower, &["япон", "пагод", "чайн", "japan", "pagoda", "tea"])
    {
        MaterialPalette::preset(9)
    } else if contains_any(&lower, &["бамбук", "джунгл", "тропик", "bamboo", "jungle", "tropical"])
        && !contains_any(&lower, &["япон", "пагод", "чайн", "japan", "pagoda", "tea"])
    {
        MaterialPalette::preset(11)
    } else if contains_any(&lower, &["маяк", "lighthouse"]) {
        let mut p = MaterialPalette::preset(4);
        p.name = "Маяк (белый бетон и камень)".into();
        p.foundation = "minecraft:stone_bricks".into();
        p.wall = "minecraft:white_concrete".into();
        p.wall_accent = "minecraft:red_wool".into();
        p.light = "minecraft:sea_lantern".into();
        p
    } else if theme == StructureTheme::Pyramid {
        let mut p = MaterialPalette::preset(6);
        if style == BuildStyle::Gothic {
            p.name = "Обсидиановый зиккурат".into();
            p.foundation = "minecraft:polished_blackstone_bricks".into();
            p.wall = "minecraft:polished_blackstone".into();
            p.wall_accent = "minecraft:chiseled_polished_blackstone".into();
            p.floor = "minecraft:gilded_blackstone".into();
            p.light = "minecraft:soul_lantern".into();
        } else {
            p.name = "Пирамида (песчаник и золото)".into();
            p.foundation = "minecraft:cut_sandstone".into();
            p.wall = "minecraft:smooth_sandstone".into();
            p.wall_accent = "minecraft:chiseled_sandstone".into();
            p.trim_wall = "minecraft:sandstone_wall".into();
            p.roof_block = "minecraft:gold_block".into();
            p.light = "minecraft:lantern".into();
        }
        p
    } else if theme == StructureTheme::Observatory {
        let mut p = MaterialPalette::preset(if style == BuildStyle::Steampunk { 18 } else { 4 });
        p.name = "Обсерватория (медь, кварц и сланец)".into();
        p.foundation = "minecraft:deepslate_bricks".into();
        p.wall = "minecraft:calcite".into();
        p.wall_accent = "minecraft:polished_diorite".into();
        p.roof_block = "minecraft:oxidized_cut_copper".into();
        p.roof_stairs = "minecraft:oxidized_cut_copper_stairs".into();
        p.roof_slab = "minecraft:oxidized_cut_copper_slab".into();
        p.light = "minecraft:copper_lantern".into();
        p
    } else if theme == StructureTheme::ClockTower {
        let mut p = MaterialPalette::preset(0);
        p.name = "Часовая башня (кирпич и медь)".into();
        p.foundation = "minecraft:stone_bricks".into();
        p.wall = "minecraft:mud_bricks".into();
        p.wall_accent = "minecraft:cut_sandstone".into();
        p.frame = "minecraft:spruce_log".into();
        p.roof_block = "minecraft:oxidized_cut_copper".into();
        p.roof_stairs = "minecraft:oxidized_cut_copper_stairs".into();
        p.roof_slab = "minecraft:oxidized_cut_copper_slab".into();
        p.light = "minecraft:ochre_froglight".into();
        p
    } else if theme == StructureTheme::OnsenBath {
        let mut p = MaterialPalette::preset(3);
        p.name = "Горячий источник Онсэн".into();
        p.foundation = "minecraft:mossy_cobblestone".into();
        p.wall = "minecraft:stripped_spruce_log".into();
        p.floor = "minecraft:bamboo_planks".into();
        p.light = "minecraft:sea_lantern".into();
        p
    } else if theme == StructureTheme::IshtarGate {
        let mut p = MaterialPalette::preset(5);
        p.name = "Ворота Иштар (лазурит и золото)".into();
        p.foundation = "minecraft:blue_terracotta".into();
        p.wall = "minecraft:blue_concrete".into();
        p.wall_accent = "minecraft:gold_block".into();
        p.frame = "minecraft:lapis_block".into();
        p.floor = "minecraft:smooth_sandstone".into();
        p.light = "minecraft:lantern".into();
        p
    } else if theme == StructureTheme::MosquePalace {
        let mut p = MaterialPalette::preset(5);
        p.name = "Купольный дворец / Мечеть".into();
        p.foundation = "minecraft:cut_sandstone".into();
        p.wall = "minecraft:smooth_sandstone".into();
        p.wall_accent = "minecraft:cyan_terracotta".into();
        p.roof_block = "minecraft:dark_prismarine".into();
        p.roof_stairs = "minecraft:dark_prismarine_stairs".into();
        p.roof_slab = "minecraft:dark_prismarine_slab".into();
        p.light = "minecraft:sea_lantern".into();
        p
    } else if matches!(theme, StructureTheme::Skyscraper | StructureTheme::ModernStadium) {
        let mut p = MaterialPalette::preset(4);
        p.name = "Хай-тек сталь и стекло".into();
        p.foundation = "minecraft:gray_concrete".into();
        p.wall = "minecraft:light_gray_concrete".into();
        p.wall_accent = "minecraft:white_concrete".into();
        p.window = "minecraft:light_blue_stained_glass".into();
        p.frame = "minecraft:iron_block".into();
        p.light = "minecraft:sea_lantern".into();
        p
    } else if theme == StructureTheme::LaunchpadRocket {
        let mut p = MaterialPalette::preset(4);
        p.name = "Космодром / Ракета".into();
        p.foundation = "minecraft:gray_concrete".into();
        p.wall = "minecraft:white_concrete".into();
        p.wall_accent = "minecraft:orange_terracotta".into();
        p.frame = "minecraft:iron_block".into();
        p.light = "minecraft:sea_lantern".into();
        p
    } else if theme == StructureTheme::MageTower {
        let mut p = MaterialPalette::preset(12);
        p.name = "Башня Архимага (аметист и пурпур)".into();
        p.foundation = "minecraft:polished_blackstone_bricks".into();
        p.wall = "minecraft:deepslate_bricks".into();
        p.wall_accent = "minecraft:amethyst_block".into();
        p.roof_block = "minecraft:purpur_block".into();
        p.roof_stairs = "minecraft:purpur_stairs".into();
        p.roof_slab = "minecraft:purpur_slab".into();
        p.light = "minecraft:soul_lantern".into();
        p
    } else if matches!(theme, StructureTheme::HobbitHole | StructureTheme::MushroomHouse) {
        let mut p = MaterialPalette::preset(0);
        p.name = "Сказочный коттедж".into();
        p.foundation = "minecraft:cobblestone".into();
        p.wall = "minecraft:oak_planks".into();
        p.frame = "minecraft:oak_log".into();
        p.leaves = "minecraft:flowering_azalea_leaves".into();
        p.light = "minecraft:lantern".into();
        p
    } else if theme == StructureTheme::DwarvenHall {
        let mut p = MaterialPalette::preset(3);
        p.name = "Дварфийские чертоги".into();
        p.foundation = "minecraft:polished_blackstone_bricks".into();
        p.wall = "minecraft:deepslate_bricks".into();
        p.wall_accent = "minecraft:gilded_blackstone".into();
        p.frame = "minecraft:polished_basalt".into();
        p.light = "minecraft:lantern".into();
        p
    } else if theme == StructureTheme::WaterShrine {
        let mut p = MaterialPalette::preset(2);
        p.name = "Святилище Тории на воде".into();
        p.foundation = "minecraft:stone_bricks".into();
        p.wall = "minecraft:white_terracotta".into();
        p.wall_accent = "minecraft:crimson_planks".into();
        p.frame = "minecraft:crimson_stem".into();
        p.roof_block = "minecraft:deepslate_tiles".into();
        p.light = "minecraft:lantern".into();
        p
    } else if contains_any(&lower, &["теплиц", "оранжере", "greenhouse", "conservatory"]) {
        let mut p = MaterialPalette::preset(0);
        p.name = "Стеклянная теплица".into();
        p.foundation = "minecraft:stone_bricks".into();
        p.wall = "minecraft:glass".into();
        p.frame = "minecraft:oak_log".into();
        p.window = "minecraft:glass_pane".into();
        p.leaves = "minecraft:flowering_azalea_leaves".into();
        p
    } else if matches!(theme, StructureTheme::Colosseum | StructureTheme::TriumphalArch | StructureTheme::Pantheon | StructureTheme::Parthenon) || style == BuildStyle::Classical {
        MaterialPalette::preset(16)
    } else if requested_style == BuildStyle::Auto
        && contains_any(&lower, &["берёз", "берез", "birch"])
    {
        MaterialPalette::preset(8)
    } else {
        MaterialPalette::preset_for_style(style)
    };

    // Explicit wood type overrides (skip if description splits 1st & 2nd floor materials explicitly).
    let (floor1_ovr, floor2_ovr) = extract_floor_wall_overrides(&lower);
    let has_split_floors = floor1_ovr.is_some() && floor2_ovr.is_some();
    if let Some(ref f1) = floor1_ovr
        && has_split_floors
    {
        if f1.contains("brick") || f1.contains("stone") || f1.contains("cobble") || f1.contains("deepslate") {
            pal.foundation = f1.clone();
        }
        pal.wall = f1.clone();
    }
    if let Some(ref f2) = floor2_ovr
        && has_split_floors
    {
        pal.wall_upper = f2.clone();
    }
    if has_split_floors {
        // Preserve ground floor base without letting upper timber clause clobber foundation masonry.
    } else if has_pair(&lower, &["бледн"], &["дуб", "дерев", "стен", "дом"]) || contains_any(&lower, &["pale oak"]) {
        apply_wood_palette(&mut pal, "pale_oak");
    } else if has_pair(&lower, &["темн", "тёмн", "черн", "чёрн"], &["дуб"]) || contains_any(&lower, &["dark oak"]) {
        apply_wood_palette(&mut pal, "dark_oak");
    } else if contains_any(&lower, &["берёз", "берез", "birch"]) {
        apply_wood_palette(&mut pal, "birch");
    } else if contains_any(&lower, &["елов", "из ели", "ельн", "хвой", "spruce"]) {
        apply_wood_palette(&mut pal, "spruce");
    } else if contains_any(&lower, &["вишн", "сакур", "cherry", "sakura"]) {
        apply_wood_palette(&mut pal, "cherry");
    } else if contains_any(&lower, &["акац", "acacia"]) {
        apply_wood_palette(&mut pal, "acacia");
    } else if contains_any(&lower, &["джунгл", "тропич", "jungle"]) {
        apply_wood_palette(&mut pal, "jungle");
    } else if contains_any(&lower, &["мангр", "mangrove"]) {
        apply_wood_palette(&mut pal, "mangrove");
    } else if contains_any(&lower, &["бамбук", "bamboo"]) {
        apply_wood_palette(&mut pal, "bamboo");
    } else if contains_any(&lower, &["багров", "crimson"]) {
        apply_wood_palette(&mut pal, "crimson");
    } else if contains_any(&lower, &["искаж", "warped"]) {
        apply_wood_palette(&mut pal, "warped");
    } else if contains_any(&lower, &["дубов", "из дуба", "oak"]) {
        apply_wood_palette(&mut pal, "oak");
    }

    // Specific masonry / wall / foundation overrides.
    if contains_any(&lower, &["булыж", "cobble"]) {
        pal.foundation = if contains_any(&lower, &["мхом", "мшист", "mossy"]) {
            "minecraft:mossy_cobblestone".into()
        } else {
            "minecraft:cobblestone".into()
        };
        pal.trim_wall = "minecraft:cobblestone_wall".into();
        if contains_any(&lower, &["булыжных стен", "из булыж", "cobble wall", "cobble house", "cobblestone house"]) {
            pal.wall = "minecraft:cobblestone".into();
        }
    } else if contains_any(&lower, &["мхом", "мшист", "mossy", "зарос", "overgrown"]) {
        pal.wall_accent = "minecraft:mossy_stone_bricks".into();
        pal.trim_wall = "minecraft:mossy_stone_brick_wall".into();
    }

    if contains_any(&lower, &["сланц", "сланец", "deepslate", "slate"]) {
        pal.foundation = "minecraft:deepslate_bricks".into();
        pal.wall_accent = "minecraft:cracked_deepslate_bricks".into();
        pal.trim_wall = "minecraft:deepslate_brick_wall".into();
        if contains_any(&lower, &["из сланц", "сланцевых стен", "сланцевый дом", "глубокого сланц", "deepslate wall", "deepslate house", "slate house"]) {
            pal.wall = "minecraft:deepslate_tiles".into();
            pal.frame = "minecraft:polished_deepslate".into();
        }
    }
    if contains_any(&lower, &["чернит", "чернокам", "blackstone", "обсидиан"]) {
        pal.foundation = "minecraft:polished_blackstone_bricks".into();
        pal.trim_wall = "minecraft:polished_blackstone_brick_wall".into();
        pal.roof_block = "minecraft:polished_blackstone_bricks".into();
        pal.roof_stairs = "minecraft:polished_blackstone_brick_stairs".into();
        pal.roof_slab = "minecraft:polished_blackstone_brick_slab".into();
        if contains_any(&lower, &["из чернит", "из чернокам", "blackstone"]) {
            pal.wall = "minecraft:polished_blackstone_bricks".into();
        }
    }
    if contains_any(&lower, &["туф", "tuff"]) {
        pal.foundation = "minecraft:tuff_bricks".into();
        pal.trim_wall = "minecraft:tuff_brick_wall".into();
        pal.wall_accent = "minecraft:chiseled_tuff_bricks".into();
        if contains_any(&lower, &["из туф", "туфов", "tuff wall", "tuff house", "tuff brick"]) {
            pal.wall = "minecraft:tuff_bricks".into();
            pal.frame = "minecraft:polished_tuff".into();
        }
    }
    if contains_any(&lower, &["кварц", "мрамор", "quartz", "marble"]) {
        pal.wall = "minecraft:smooth_quartz".into();
        pal.wall_accent = "minecraft:quartz_bricks".into();
        pal.frame = "minecraft:quartz_pillar".into();
        pal.foundation = "minecraft:polished_diorite".into();
        pal.floor = "minecraft:smooth_quartz".into();
    } else if contains_any(&lower, &["кальцит", "мелов", "известняк", "бел стен", "белые стен", "белый дом", "calcite", "white wall", "limestone"]) {
        pal.wall = "minecraft:calcite".into();
        pal.wall_accent = "minecraft:white_terracotta".into();
    } else if contains_any(&lower, &["бетон", "concrete"]) {
        pal.wall = if contains_any(&lower, &["черн", "чёрн", "темн", "тёмн", "black", "dark"]) {
            "minecraft:black_concrete".into()
        } else if contains_any(&lower, &["сер", "gray", "grey"]) {
            "minecraft:gray_concrete".into()
        } else if contains_any(&lower, &["голуб", "син", "cyan", "blue"]) {
            "minecraft:cyan_terracotta".into()
        } else {
            "minecraft:white_concrete".into()
        };
        pal.wall_accent = "minecraft:light_gray_concrete".into();
        pal.frame = "minecraft:gray_concrete".into();
    } else if contains_any(&lower, &["песчан", "sandstone"]) {
        if contains_any(&lower, &["красн", "рыж", "red"]) {
            pal.foundation = "minecraft:cut_red_sandstone".into();
            pal.wall = "minecraft:red_sandstone".into();
            pal.trim_wall = "minecraft:red_sandstone_wall".into();
        } else {
            pal.foundation = "minecraft:cut_sandstone".into();
            pal.wall = "minecraft:sandstone".into();
            pal.wall_accent = "minecraft:smooth_sandstone".into();
            pal.trim_wall = "minecraft:sandstone_wall".into();
        }
    } else if contains_any(&lower, &["смол", "resin"]) {
        pal.wall = "minecraft:resin_bricks".into();
        pal.trim_wall = "minecraft:resin_brick_wall".into();
        pal.roof_block = "minecraft:resin_bricks".into();
        pal.roof_stairs = "minecraft:resin_brick_stairs".into();
        pal.roof_slab = "minecraft:resin_brick_slab".into();
    } else if contains_any(&lower, &["кирпич", "brick"]) {
        if has_pair(&lower, &["кирпич", "brick"], &["крыш", "roof"])
            && !contains_any(&lower, &["из кирпич", "кирпичный дом", "кирпичные стен", "brick wall", "brick house", "glazed brick"])
        {
            pal.roof_block = "minecraft:bricks".into();
            pal.roof_stairs = "minecraft:brick_stairs".into();
            pal.roof_slab = "minecraft:brick_slab".into();
        } else {
            pal.foundation = "minecraft:stone_bricks".into();
            pal.wall = "minecraft:bricks".into();
            pal.wall_accent = "minecraft:terracotta".into();
            pal.trim_wall = "minecraft:brick_wall".into();
        }
    } else if contains_any(&lower, &["саман", "глин", "терракот", "mud", "terracotta", "adobe"]) {
        pal.wall = if contains_any(&lower, &["бел", "white"]) {
            "minecraft:white_terracotta".into()
        } else if contains_any(&lower, &["голуб", "син", "cyan", "blue"]) {
            "minecraft:cyan_terracotta".into()
        } else if contains_any(&lower, &["рыж", "оранж", "красн", "red", "orange"]) {
            "minecraft:terracotta".into()
        } else {
            "minecraft:mud_bricks".into()
        };
        pal.trim_wall = "minecraft:mud_brick_wall".into();
    } else if contains_any(&lower, &["гранит", "granite"]) {
        pal.foundation = "minecraft:polished_granite".into();
        pal.trim_wall = "minecraft:granite_wall".into();
    } else if contains_any(&lower, &["диорит", "diorite"]) {
        pal.foundation = "minecraft:polished_diorite".into();
        pal.trim_wall = "minecraft:diorite_wall".into();
    } else if contains_any(&lower, &["андезит", "andesite"]) {
        pal.foundation = "minecraft:polished_andesite".into();
        pal.trim_wall = "minecraft:andesite_wall".into();
    } else if contains_any(&lower, &["базальт", "basalt"]) {
        pal.foundation = "minecraft:polished_basalt".into();
        pal.frame = "minecraft:polished_basalt".into();
    } else if contains_any(&lower, &["каменн", "из камн", "stone brick", "stone house", "stone wall"]) {
        pal.foundation = "minecraft:stone_bricks".into();
        pal.wall = "minecraft:stone_bricks".into();
        pal.wall_accent = "minecraft:cracked_stone_bricks".into();
        pal.trim_wall = "minecraft:stone_brick_wall".into();
    }

    // Explicit Roof material & color overrides.
    if contains_any(&lower, &["медн", "медь", "из меди", "copper"]) {
        if contains_any(&lower, &["окисл", "зелен", "зелён", "бирюз", "oxidized", "patina"]) {
            pal.roof_block = "minecraft:oxidized_cut_copper".into();
            pal.roof_stairs = "minecraft:oxidized_cut_copper_stairs".into();
            pal.roof_slab = "minecraft:oxidized_cut_copper_slab".into();
        } else if contains_any(&lower, &["состар", "exposed", "weathered"]) {
            pal.roof_block = "minecraft:exposed_cut_copper".into();
            pal.roof_stairs = "minecraft:exposed_cut_copper_stairs".into();
            pal.roof_slab = "minecraft:exposed_cut_copper_slab".into();
        } else {
            pal.roof_block = "minecraft:cut_copper".into();
            pal.roof_stairs = "minecraft:cut_copper_stairs".into();
            pal.roof_slab = "minecraft:cut_copper_slab".into();
        }
        pal.light = "minecraft:copper_lantern".into();
    } else if contains_any(&lower, &["черепиц", "tile roof"]) || has_pair(&lower, &["красн", "кирпич", "red", "brick"], &["крыш", "roof"]) {
        pal.roof_block = "minecraft:bricks".into();
        pal.roof_stairs = "minecraft:brick_stairs".into();
        pal.roof_slab = "minecraft:brick_slab".into();
    } else if has_pair(&lower, &["зел", "зелён", "бирюз", "изумруд", "green", "teal", "cyan", "emerald"], &["крыш", "roof"]) {
        pal.roof_block = "minecraft:dark_prismarine".into();
        pal.roof_stairs = "minecraft:dark_prismarine_stairs".into();
        pal.roof_slab = "minecraft:dark_prismarine_slab".into();
    } else if has_pair(&lower, &["син", "голуб", "лазур", "blue", "azure"], &["крыш", "roof"]) {
        pal.roof_block = "minecraft:warped_planks".into();
        pal.roof_stairs = "minecraft:warped_stairs".into();
        pal.roof_slab = "minecraft:warped_slab".into();
    } else if has_pair(&lower, &["фиолет", "сирен", "пурпур", "маг", "purple", "violet", "magenta", "purpur"], &["крыш", "roof"]) {
        pal.roof_block = "minecraft:purpur_block".into();
        pal.roof_stairs = "minecraft:purpur_stairs".into();
        pal.roof_slab = "minecraft:purpur_slab".into();
    } else if has_pair(&lower, &["розов", "pink", "cherry"], &["крыш", "roof"]) {
        pal.roof_block = "minecraft:cherry_planks".into();
        pal.roof_stairs = "minecraft:cherry_stairs".into();
        pal.roof_slab = "minecraft:cherry_slab".into();
    } else if has_pair(&lower, &["бел", "кварц", "снеж", "white", "quartz", "snow"], &["крыш", "roof"]) {
        pal.roof_block = "minecraft:smooth_quartz".into();
        pal.roof_stairs = "minecraft:smooth_quartz_stairs".into();
        pal.roof_slab = "minecraft:smooth_quartz_slab".into();
    } else if has_pair(&lower, &["жёлт", "желт", "золот", "песч", "yellow", "gold", "sand"], &["крыш", "roof"]) {
        pal.roof_block = "minecraft:bamboo_planks".into();
        pal.roof_stairs = "minecraft:bamboo_stairs".into();
        pal.roof_slab = "minecraft:bamboo_slab".into();
    } else if has_pair(&lower, &["чёрн", "черн", "темн", "тёмн", "сланц", "black", "dark", "slate", "deepslate"], &["крыш", "roof"]) {
        pal.roof_block = "minecraft:deepslate_tiles".into();
        pal.roof_stairs = "minecraft:deepslate_tile_stairs".into();
        pal.roof_slab = "minecraft:deepslate_tile_slab".into();
    }

    // Window / stained glass overrides.
    if has_pair(&lower, &["голуб", "син", "бирюз", "blue", "cyan", "azure"], &["окн", "стекл", "витраж", "window", "glass", "pane"]) {
        pal.window = "minecraft:light_blue_stained_glass_pane".into();
    } else if has_pair(&lower, &["фиолет", "сирен", "purple", "violet"], &["окн", "стекл", "витраж", "window", "glass", "pane"]) {
        pal.window = "minecraft:purple_stained_glass_pane".into();
    } else if has_pair(&lower, &["розов", "pink"], &["окн", "стекл", "витраж", "window", "glass", "pane"]) {
        pal.window = "minecraft:pink_stained_glass_pane".into();
    } else if has_pair(&lower, &["красн", "red"], &["окн", "стекл", "витраж", "window", "glass", "pane"]) {
        pal.window = "minecraft:red_stained_glass_pane".into();
    } else if has_pair(&lower, &["жёлт", "желт", "золот", "yellow", "gold"], &["окн", "стекл", "витраж", "window", "glass", "pane"]) {
        pal.window = "minecraft:yellow_stained_glass_pane".into();
    } else if has_pair(&lower, &["зел", "зелён", "green"], &["окн", "стекл", "витраж", "window", "glass", "pane"]) {
        pal.window = "minecraft:green_stained_glass_pane".into();
    } else if has_pair(&lower, &["чёрн", "черн", "темн", "тёмн", "тонир", "black", "dark", "tinted"], &["окн", "стекл", "glass", "window"]) {
        pal.window = "minecraft:tinted_glass".into();
    } else if contains_any(&lower, &["витраж", "stained glass"]) && pal.window == "minecraft:glass_pane" {
        pal.window = "minecraft:yellow_stained_glass_pane".into();
    } else if has_pair(&lower, &["решёт", "решет", "iron"], &["окн", "window", "grate", "bars"]) {
        pal.window = "minecraft:iron_bars".into();
    } else if has_pair(&lower, &["медн", "copper"], &["окн", "решёт", "решет", "window", "grate"]) {
        pal.window = "minecraft:copper_grate".into();
    }

    // Lighting overrides.
    if contains_any(&lower, &["душ", "синим ог", "синий ог", "синий свет", "голубой свет", "soul", "blue flame"]) {
        pal.light = "minecraft:soul_lantern".into();
    } else if contains_any(&lower, &["морск", "аква", "sea lantern"]) {
        pal.light = "minecraft:sea_lantern".into();
    } else if contains_any(&lower, &["грибосвет", "shroomlight"]) {
        pal.light = "minecraft:shroomlight".into();
    } else if contains_any(&lower, &["глоустоун", "светокам", "glowstone"]) {
        pal.light = "minecraft:glowstone".into();
    } else if contains_any(&lower, &["ламп", "редстоун", "redstone lamp"]) {
        pal.light = "minecraft:redstone_lamp".into();
    } else if contains_any(&lower, &["стерж", "неон", "end rod", "neon"]) {
        pal.light = "minecraft:end_rod".into();
    } else if contains_any(&lower, &["золот свет", "янтар", "тепл свет", "тёпл свет", "ochre", "amber light", "warm light"]) {
        pal.light = "minecraft:ochre_froglight".into();
    }

    if !has_split_floors && pal.wall_upper != pal.wall {
        // Keep uniform single-material wall look when text explicitly overrode primary wall.
        pal.wall_upper = pal.wall.clone();
    }

    pal
}

/// Backward-compatible wrapper inferring palette purely from text description (default seed 42).
pub fn infer_palette_with_style(requested_style: BuildStyle, description: &str) -> MaterialPalette {
    infer_palette_with_style_and_seed(requested_style, description, 42)
}

/// Backward-compatible wrapper inferring palette purely from text description.
pub fn infer_palette_from_text(description: &str) -> MaterialPalette {
    infer_palette_with_style(BuildStyle::Auto, description)
}
