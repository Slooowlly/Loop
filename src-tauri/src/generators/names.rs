#![allow(dead_code)]

// INTEGRACAO FUTURA:
// driver.rs::generate_for_category() deve chamar generate_pilot_identity()
// para obter nome, nacionalidade e genero realistas.
// Hoje driver.rs usa nomes placeholder - sera atualizado quando o Wizard for implementado.

use std::collections::HashSet;

use rand::Rng;

use crate::generators::nationality::{format_nationality, random_nationality};

#[derive(Debug, Clone)]
pub struct NamePool {
    pub nationality_id: &'static str,
    pub nomes_masculinos: &'static [&'static str],
    pub nomes_femininos: &'static [&'static str],
    pub sobrenomes: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PilotIdentity {
    pub nome_completo: String,
    pub primeiro_nome: String,
    pub sobrenome: String,
    pub nacionalidade_id: String,
    pub nacionalidade_label: String,
    pub genero: String,
}

static GB_MALE: &[&str] = &[
    "James",
    "Thomas",
    "Oliver",
    "William",
    "George",
    "Harry",
    "Jack",
    "Charlie",
    "Daniel",
    "Samuel",
    "Joseph",
    "Benjamin",
    "Henry",
    "Edward",
    "Alexander",
    "Matthew",
    "Ryan",
    "Nathan",
    "Luke",
    "Adam",
    "Connor",
    "Ethan",
    "Owen",
    "Jake",
    "Dylan",
    "Kieran",
    "Liam",
    "Ross",
    "Nathaniel",
    "Patrick",
];
static GB_FEMALE: &[&str] = &[
    "Emily",
    "Charlotte",
    "Sophie",
    "Hannah",
    "Jessica",
    "Olivia",
    "Grace",
    "Amelia",
];
static GB_LAST: &[&str] = &[
    "Smith", "Jones", "Williams", "Brown", "Taylor", "Wilson", "Davies", "Evans", "Thomas",
    "Roberts", "Walker", "Wright", "Turner", "Hill", "Clarke", "Mitchell", "Cooper", "Ward",
    "Morris", "King", "Green", "Baker", "Hall", "Wood", "Harris", "Clark", "Harrison", "Scott",
    "Edwards", "Murray",
];

static DE_MALE: &[&str] = &[
    "Lukas",
    "Niklas",
    "Florian",
    "Jonas",
    "Tobias",
    "Felix",
    "Moritz",
    "Tim",
    "Julian",
    "Leon",
    "Maximilian",
    "Sebastian",
    "Johannes",
    "Daniel",
    "David",
    "Philipp",
    "Matthias",
    "Andreas",
    "Simon",
    "Marvin",
    "Kevin",
    "Dennis",
    "Dominik",
    "Fabian",
    "Robin",
    "Benedikt",
    "Kai",
    "Christian",
    "Jan",
    "Nico",
];
static DE_FEMALE: &[&str] = &[
    "Anna", "Laura", "Sophie", "Leonie", "Lisa", "Marie", "Julia", "Lena",
];
static DE_LAST: &[&str] = &[
    "Muller",
    "Schmidt",
    "Schneider",
    "Fischer",
    "Weber",
    "Meyer",
    "Wagner",
    "Becker",
    "Hoffmann",
    "Schulz",
    "Koch",
    "Bauer",
    "Richter",
    "Klein",
    "Wolf",
    "Neumann",
    "Schroder",
    "Braun",
    "Hartmann",
    "Werner",
    "Krause",
    "Meier",
    "Lehmann",
    "Schmid",
    "Schulze",
    "Maier",
    "Kohler",
    "Herrmann",
    "Konig",
    "Walter",
];

static FR_MALE: &[&str] = &[
    "Lucas", "Nathan", "Jules", "Louis", "Hugo", "Theo", "Antoine", "Maxime", "Adrien", "Clement",
    "Matthieu", "Julien", "Bastien", "Remy", "Alexis", "Nicolas", "Gabriel", "Romain", "Quentin",
    "Vincent", "Benoit", "Damien", "Thomas", "Arthur", "Martin",
];
static FR_FEMALE: &[&str] = &["Camille", "Emma", "Chloe", "Lucie", "Manon", "Lea"];
static FR_LAST: &[&str] = &[
    "Martin", "Bernard", "Thomas", "Petit", "Robert", "Richard", "Durand", "Dubois", "Moreau",
    "Laurent", "Simon", "Michel", "Lefebvre", "Leroy", "Roux", "David", "Bertrand", "Morel",
    "Fournier", "Girard", "Andre", "Mercier", "Dupont", "Lambert", "Bonnet",
];

static IT_MALE: &[&str] = &[
    "Luca",
    "Matteo",
    "Andrea",
    "Giovanni",
    "Marco",
    "Davide",
    "Simone",
    "Paolo",
    "Stefano",
    "Riccardo",
    "Alessio",
    "Francesco",
    "Daniele",
    "Christian",
    "Gabriele",
    "Nicolo",
    "Emanuele",
    "Federico",
    "Antonio",
    "Filippo",
    "Roberto",
    "Massimo",
    "Claudio",
    "Tommaso",
    "Enrico",
];
static IT_FEMALE: &[&str] = &["Giulia", "Chiara", "Martina", "Sara", "Elena", "Francesca"];
static IT_LAST: &[&str] = &[
    "Villa", "Russo", "Ferraro", "Esposito", "Bianchi", "Romano", "Colombo", "Ricci", "Marino",
    "Greco", "Bruno", "Gallo", "Conti", "DeLuca", "Mancini", "Costa", "Giordano", "Rinaldi",
    "Lombardi", "Moretti", "Barbieri", "Fontana", "Caruso", "Leone", "Santoro",
];

static ES_MALE: &[&str] = &[
    "Alejandro",
    "Pablo",
    "Diego",
    "Javier",
    "Alvaro",
    "Adrian",
    "Ivan",
    "Hector",
    "Ruben",
    "Victor",
    "Raul",
    "Marcos",
    "Sergio",
    "Miguel",
    "Andres",
    "Jorge",
    "Guillermo",
    "Julian",
    "Tomas",
    "Daniel",
    "Nicolas",
    "Bruno",
    "Gabriel",
    "Joel",
    "Manuel",
];
static ES_FEMALE: &[&str] = &["Lucia", "Marta", "Elena", "Paula", "Irene", "Carmen"];
static ES_LAST: &[&str] = &[
    "Garcia",
    "Martinez",
    "Lopez",
    "Sanchez",
    "Perez",
    "Gomez",
    "Martin",
    "Jimenez",
    "Ruiz",
    "Hernandez",
    "Diaz",
    "Moreno",
    "Munoz",
    "Alvarez",
    "Romero",
    "Castro",
    "Gutierrez",
    "Navarro",
    "Torres",
    "Dominguez",
    "Vazquez",
    "Ramos",
    "Gil",
    "Serrano",
    "Blanco",
];

static BR_MALE: &[&str] = &[
    "Lucas",
    "Gabriel",
    "Rafael",
    "Matheus",
    "Gustavo",
    "Felipe",
    "Pedro",
    "Thiago",
    "Bruno",
    "Andre",
    "Marcos",
    "Leonardo",
    "Henrique",
    "Vinicius",
    "Eduardo",
    "Rodrigo",
    "Caio",
    "Diego",
    "Renato",
    "Fernando",
    "Paulo",
    "Marcelo",
    "Igor",
    "Leandro",
    "Alexandre",
];
static BR_FEMALE: &[&str] = &[
    "Juliana", "Camila", "Beatriz", "Fernanda", "Larissa", "Carolina",
];
static BR_LAST: &[&str] = &[
    "Silva",
    "Santos",
    "Oliveira",
    "Souza",
    "Pereira",
    "Costa",
    "Ferreira",
    "Almeida",
    "Carvalho",
    "Ribeiro",
    "Gomes",
    "Martins",
    "Rocha",
    "Lima",
    "Araujo",
    "Fernandes",
    "Barbosa",
    "Cardoso",
    "Moreira",
    "Nunes",
    "Cavalcanti",
    "Monteiro",
    "Teixeira",
    "Mendes",
    "Correia",
];

static NL_MALE: &[&str] = &[
    "Daan", "Milan", "Sem", "Luuk", "Bram", "Jesse", "Stijn", "Niels", "Thijs", "Joris", "Tom",
    "Koen", "Sven", "Ruben", "Lars", "Pieter", "Willem", "Timo", "Bas", "Cas",
];
static NL_FEMALE: &[&str] = &["Emma", "Sanne", "Lisa", "Noa", "Julia"];
static NL_LAST: &[&str] = &[
    "deJong",
    "Jansen",
    "deVries",
    "vanDijk",
    "Bakker",
    "Janssen",
    "Visser",
    "Smit",
    "Meijer",
    "deBoer",
    "Mulder",
    "deGroot",
    "Bos",
    "Vos",
    "Peters",
    "Hendriks",
    "vanLeeuwen",
    "Dekker",
    "Schouten",
    "Kramer",
];

static AU_MALE: &[&str] = &[
    "Liam", "Noah", "Mason", "Cooper", "Hudson", "Eli", "Isaac", "Xavier", "Jordan", "Logan",
    "Mitchell", "Zachary", "Flynn", "Bailey", "Tyler", "Aiden", "Connor", "Blake", "Jayden",
    "Ashton",
];
static AU_FEMALE: &[&str] = &["Mia", "Chloe", "Zoe", "Ella", "Ruby"];
static AU_LAST: &[&str] = &[
    "Smith", "Jones", "Taylor", "Brown", "Wilson", "Anderson", "Thomas", "White", "Martin",
    "Thompson", "Walker", "Young", "Allen", "Hall", "King", "Wright", "Scott", "Green", "Mitchell",
    "Campbell",
];

static JP_MALE: &[&str] = &[
    "Takumi", "Haruto", "Yuto", "Sota", "Riku", "Kaito", "Haruki", "Ren", "Kota", "Daiki", "Shota",
    "Yuki", "Ryota", "Kenji", "Hiroshi", "Naoki", "Tatsuya", "Sho", "Kenta", "Akira",
];
static JP_FEMALE: &[&str] = &["Yui", "Hana", "Sakura", "Aoi", "Rin"];
static JP_LAST: &[&str] = &[
    "Tanaka",
    "Suzuki",
    "Takahashi",
    "Watanabe",
    "Ito",
    "Yamamoto",
    "Nakamura",
    "Kobayashi",
    "Kato",
    "Yoshida",
    "Yamada",
    "Sasaki",
    "Matsumoto",
    "Inoue",
    "Kimura",
    "Shimizu",
    "Hayashi",
    "Saito",
    "Mori",
    "Ikeda",
];

static US_MALE: &[&str] = &[
    "Ethan", "Noah", "Mason", "Caleb", "Wyatt", "Logan", "Austin", "Hunter", "Cameron", "Parker",
    "Colton", "Brody", "Evan", "Tyler", "Brandon", "Austin", "Gavin", "Jordan", "Derek", "Preston",
];
static US_FEMALE: &[&str] = &["Madison", "Abigail", "Hailey", "Avery", "Natalie"];
static US_LAST: &[&str] = &[
    "Smith",
    "Johnson",
    "Williams",
    "Brown",
    "Jones",
    "Miller",
    "Davis",
    "Garcia",
    "Rodriguez",
    "Martinez",
    "Anderson",
    "Taylor",
    "Thomas",
    "Moore",
    "Jackson",
    "Martin",
    "Lee",
    "Perez",
    "Thompson",
    "White",
];

static MX_MALE: &[&str] = &[
    "Luis", "Jorge", "Miguel", "Adrian", "Eduardo", "Emilio", "Ricardo", "Hugo", "Arturo", "Ramon",
    "Julio", "Cesar", "Omar", "Victor", "Manuel",
];
static MX_FEMALE: &[&str] = &["Valeria", "Daniela", "Mariana", "Ximena"];
static MX_LAST: &[&str] = &[
    "Hernandez",
    "Garcia",
    "Martinez",
    "Lopez",
    "Gonzalez",
    "Perez",
    "Rodriguez",
    "Sanchez",
    "Ramirez",
    "Flores",
    "Gomez",
    "Diaz",
    "Reyes",
    "Cruz",
    "Morales",
];

static AR_MALE: &[&str] = &[
    "Agustin", "Matias", "Tomas", "Joaquin", "Franco", "Luciano", "Bruno", "Nicolas", "Gonzalo",
    "Leandro", "Damian", "Emiliano", "Ramiro", "Facundo", "Esteban",
];
static AR_FEMALE: &[&str] = &["Sofia", "Martina", "Julieta", "Agustina"];
static AR_LAST: &[&str] = &[
    "Gomez",
    "Fernandez",
    "Lopez",
    "Diaz",
    "Martinez",
    "Perez",
    "Romero",
    "Sosa",
    "Alvarez",
    "Torres",
    "Ruiz",
    "Suarez",
    "Benitez",
    "Acosta",
    "Herrera",
];

static FI_MALE: &[&str] = &[
    "Mikael", "Joonas", "Antti", "Aleksi", "Eetu", "Oskari", "Ville", "Juho", "Mikko", "Sami",
    "Toni", "Jesse", "Lauri", "Arttu", "Petri",
];
static FI_FEMALE: &[&str] = &["Aino", "Emilia", "Laura", "Sanni"];
static FI_LAST: &[&str] = &[
    "Korhonen",
    "Virtanen",
    "Maki",
    "Nieminen",
    "Makinen",
    "Hamalainen",
    "Laine",
    "Heikkinen",
    "Koskinen",
    "Jarvinen",
    "Lehtonen",
    "Leppanen",
    "Salonen",
    "Rantanen",
    "Karjalainen",
];

static BE_MALE: &[&str] = &[
    "Arthur", "Louis", "Maxime", "Julien", "Thomas", "Nicolas", "Benoit", "Antoine", "Cyril",
    "David", "Hugo", "Matthias", "Simon", "Cedric", "Victor",
];
static BE_FEMALE: &[&str] = &["Elise", "Julie", "Laura", "Manon"];
static BE_LAST: &[&str] = &[
    "Peeters",
    "Janssens",
    "Maes",
    "Jacobs",
    "Mertens",
    "Willems",
    "Claes",
    "Goossens",
    "Wouters",
    "DeSmet",
    "Vermeulen",
    "Dubois",
    "Lambert",
    "Leroy",
    "Noel",
];

static PT_MALE: &[&str] = &[
    "Joao", "Tiago", "Diogo", "Rui", "Miguel", "Andre", "Nuno", "Pedro", "Bruno", "Goncalo",
    "Tomas", "Afonso", "Ricardo", "Hugo", "Vasco",
];
static PT_FEMALE: &[&str] = &["Ines", "Marta", "Beatriz", "Joana"];
static PT_LAST: &[&str] = &[
    "Silva",
    "Santos",
    "Ferreira",
    "Pereira",
    "Oliveira",
    "Costa",
    "Rodrigues",
    "Martins",
    "Jesus",
    "Sousa",
    "Fernandes",
    "Goncalves",
    "Gomes",
    "Lopes",
    "Marques",
];

static CA_MALE: &[&str] = &[
    "Evan", "Mason", "Colin", "Noah", "Tyler", "Connor", "Declan", "Spencer", "Jordan", "Nathan",
    "Carter", "Brendan", "Mitchell", "Grant", "Owen",
];
static CA_FEMALE: &[&str] = &["Claire", "Sophie", "Lauren", "Megan"];
static CA_LAST: &[&str] = &[
    "Smith", "Martin", "Tremblay", "Roy", "Gagne", "Lee", "Wilson", "Brown", "Cote", "Bouchard",
    "Gauthier", "Morin", "Lavoie", "Fortin", "Fraser",
];

static AT_MALE: &[&str] = &[
    "Lukas",
    "Jonas",
    "Felix",
    "David",
    "Tobias",
    "Stefan",
    "Martin",
    "Michael",
    "Andreas",
    "Florian",
    "Dominik",
    "Fabian",
    "Julian",
    "Christoph",
    "Manuel",
];
static AT_FEMALE: &[&str] = &["Anna", "Lisa", "Julia", "Sarah"];
static AT_LAST: &[&str] = &[
    "Gruber", "Huber", "Wagner", "Pichler", "Moser", "Steiner", "Mayer", "Seidl", "Hofer", "Bauer",
    "Eder", "Fuchs", "Leitner", "Winter", "Schmid",
];

static CH_MALE: &[&str] = &[
    "Luca", "Jan", "Noah", "Simon", "Matthias", "David", "Pascal", "Fabian", "Joel", "Timo",
    "Nils", "Marco", "Jonas", "Cedric", "Adrian",
];
static CH_FEMALE: &[&str] = &["Lara", "Nina", "Lea", "Julia"];
static CH_LAST: &[&str] = &[
    "Muller",
    "Meier",
    "Schmid",
    "Keller",
    "Weber",
    "Huber",
    "Frei",
    "Brunner",
    "Baumann",
    "Zimmermann",
    "Gerber",
    "Steiner",
    "Ammann",
    "Kunz",
    "Graf",
];

static DK_MALE: &[&str] = &[
    "Mads", "Jonas", "Lasse", "Emil", "Frederik", "Rasmus", "Nikolaj", "Anders", "Kasper",
    "Mikkel", "Oliver", "Troels", "Mathias", "Jakob", "Soren",
];
static DK_FEMALE: &[&str] = &["Emma", "Clara", "Sofie", "Freja"];
static DK_LAST: &[&str] = &[
    "Jensen",
    "Nielsen",
    "Hansen",
    "Pedersen",
    "Andersen",
    "Christensen",
    "Larsen",
    "Sorensen",
    "Rasmussen",
    "Jorgensen",
    "Madsen",
    "Kristensen",
    "Olsen",
    "Thomsen",
    "Mortensen",
];

static SE_MALE: &[&str] = &[
    "Erik", "Viktor", "Anton", "Filip", "Emil", "Oskar", "Johan", "Henrik", "Ludvig", "Axel",
    "Albin", "Robin", "Marcus", "Gustav", "Simon",
];
static SE_FEMALE: &[&str] = &["Elsa", "Maja", "Alva", "Julia"];
static SE_LAST: &[&str] = &[
    "Andersson",
    "Johansson",
    "Karlsson",
    "Nilsson",
    "Eriksson",
    "Larsson",
    "Olsson",
    "Persson",
    "Svensson",
    "Gustafsson",
    "Pettersson",
    "Jonsson",
    "Jansson",
    "Hansson",
    "Bergstrom",
];

static NO_MALE: &[&str] = &[
    "Ola", "Jon", "Lars", "Magnus", "Andreas", "Emil", "Kristian", "Tobias", "Sindre", "Marius",
    "Henrik", "Vetle", "Eirik", "Martin", "Fredrik",
];
static NO_FEMALE: &[&str] = &["Ingrid", "Nora", "Emma", "Sara"];
static NO_LAST: &[&str] = &[
    "Hansen",
    "Johansen",
    "Olsen",
    "Larsen",
    "Andersen",
    "Pedersen",
    "Nilsen",
    "Kristiansen",
    "Jensen",
    "Karlsen",
    "Johnsen",
    "Pettersen",
    "Eriksen",
    "Berg",
    "Dahl",
];

static PL_MALE: &[&str] = &[
    "Jakub",
    "Marek",
    "Piotr",
    "Krzysztof",
    "Pawel",
    "Mikolaj",
    "Lukasz",
    "Tomasz",
    "Kamil",
    "Patryk",
    "Michal",
    "Adrian",
    "Dominik",
    "Marcin",
    "Wojciech",
];
static PL_FEMALE: &[&str] = &["Anna", "Katarzyna", "Magdalena", "Oliwia"];
static PL_LAST: &[&str] = &[
    "Nowak",
    "Kowalski",
    "Wisniewski",
    "Wojcik",
    "Kowalczyk",
    "Kaminski",
    "Lewandowski",
    "Zielinski",
    "Szymanski",
    "Wozniak",
    "Dabrowski",
    "Kozlowski",
    "Jankowski",
    "Mazur",
    "Krawczyk",
];

static CN_MALE: &[&str] = &[
    "Wei", "Jun", "Hao", "Tao", "Ming", "Jie", "Qiang", "Bo", "Lei", "Yong", "Peng", "Chao",
    "Jian", "Chen", "Lin",
];
static CN_FEMALE: &[&str] = &["Li", "Mei", "Xiu", "Lan"];
static CN_LAST: &[&str] = &[
    "Wang", "Li", "Zhang", "Liu", "Chen", "Yang", "Huang", "Zhao", "Wu", "Zhou", "Xu", "Sun", "Ma",
    "Zhu", "Hu",
];

static NAME_POOLS: [NamePool; 23] = [
    NamePool {
        nationality_id: "gb",
        nomes_masculinos: GB_MALE,
        nomes_femininos: GB_FEMALE,
        sobrenomes: GB_LAST,
    },
    NamePool {
        nationality_id: "de",
        nomes_masculinos: DE_MALE,
        nomes_femininos: DE_FEMALE,
        sobrenomes: DE_LAST,
    },
    NamePool {
        nationality_id: "fr",
        nomes_masculinos: FR_MALE,
        nomes_femininos: FR_FEMALE,
        sobrenomes: FR_LAST,
    },
    NamePool {
        nationality_id: "it",
        nomes_masculinos: IT_MALE,
        nomes_femininos: IT_FEMALE,
        sobrenomes: IT_LAST,
    },
    NamePool {
        nationality_id: "es",
        nomes_masculinos: ES_MALE,
        nomes_femininos: ES_FEMALE,
        sobrenomes: ES_LAST,
    },
    NamePool {
        nationality_id: "br",
        nomes_masculinos: BR_MALE,
        nomes_femininos: BR_FEMALE,
        sobrenomes: BR_LAST,
    },
    NamePool {
        nationality_id: "nl",
        nomes_masculinos: NL_MALE,
        nomes_femininos: NL_FEMALE,
        sobrenomes: NL_LAST,
    },
    NamePool {
        nationality_id: "au",
        nomes_masculinos: AU_MALE,
        nomes_femininos: AU_FEMALE,
        sobrenomes: AU_LAST,
    },
    NamePool {
        nationality_id: "jp",
        nomes_masculinos: JP_MALE,
        nomes_femininos: JP_FEMALE,
        sobrenomes: JP_LAST,
    },
    NamePool {
        nationality_id: "us",
        nomes_masculinos: US_MALE,
        nomes_femininos: US_FEMALE,
        sobrenomes: US_LAST,
    },
    NamePool {
        nationality_id: "mx",
        nomes_masculinos: MX_MALE,
        nomes_femininos: MX_FEMALE,
        sobrenomes: MX_LAST,
    },
    NamePool {
        nationality_id: "ar",
        nomes_masculinos: AR_MALE,
        nomes_femininos: AR_FEMALE,
        sobrenomes: AR_LAST,
    },
    NamePool {
        nationality_id: "fi",
        nomes_masculinos: FI_MALE,
        nomes_femininos: FI_FEMALE,
        sobrenomes: FI_LAST,
    },
    NamePool {
        nationality_id: "be",
        nomes_masculinos: BE_MALE,
        nomes_femininos: BE_FEMALE,
        sobrenomes: BE_LAST,
    },
    NamePool {
        nationality_id: "pt",
        nomes_masculinos: PT_MALE,
        nomes_femininos: PT_FEMALE,
        sobrenomes: PT_LAST,
    },
    NamePool {
        nationality_id: "ca",
        nomes_masculinos: CA_MALE,
        nomes_femininos: CA_FEMALE,
        sobrenomes: CA_LAST,
    },
    NamePool {
        nationality_id: "at",
        nomes_masculinos: AT_MALE,
        nomes_femininos: AT_FEMALE,
        sobrenomes: AT_LAST,
    },
    NamePool {
        nationality_id: "ch",
        nomes_masculinos: CH_MALE,
        nomes_femininos: CH_FEMALE,
        sobrenomes: CH_LAST,
    },
    NamePool {
        nationality_id: "dk",
        nomes_masculinos: DK_MALE,
        nomes_femininos: DK_FEMALE,
        sobrenomes: DK_LAST,
    },
    NamePool {
        nationality_id: "se",
        nomes_masculinos: SE_MALE,
        nomes_femininos: SE_FEMALE,
        sobrenomes: SE_LAST,
    },
    NamePool {
        nationality_id: "no",
        nomes_masculinos: NO_MALE,
        nomes_femininos: NO_FEMALE,
        sobrenomes: NO_LAST,
    },
    NamePool {
        nationality_id: "pl",
        nomes_masculinos: PL_MALE,
        nomes_femininos: PL_FEMALE,
        sobrenomes: PL_LAST,
    },
    NamePool {
        nationality_id: "cn",
        nomes_masculinos: CN_MALE,
        nomes_femininos: CN_FEMALE,
        sobrenomes: CN_LAST,
    },
];

pub fn get_all_name_pools() -> &'static [NamePool] {
    &NAME_POOLS
}

pub fn get_name_pool(nationality_id: &str) -> Option<&'static NamePool> {
    NAME_POOLS
        .iter()
        .find(|pool| pool.nationality_id == nationality_id)
}

pub fn generate_name(nationality_id: &str, genero: &str, rng: &mut impl Rng) -> (String, String) {
    let pool = get_name_pool(nationality_id).unwrap_or(&NAME_POOLS[0]);
    let first_names = if genero.eq_ignore_ascii_case("F") && !pool.nomes_femininos.is_empty() {
        pool.nomes_femininos
    } else {
        pool.nomes_masculinos
    };

    let first_name = first_names[rng.gen_range(0..first_names.len())].to_string();
    let last_name = pool.sobrenomes[rng.gen_range(0..pool.sobrenomes.len())].to_string();
    (first_name, last_name)
}

pub fn generate_unique_name(
    nationality_id: &str,
    genero: &str,
    existing_names: &HashSet<String>,
    rng: &mut impl Rng,
) -> (String, String) {
    for _ in 0..50 {
        let (first_name, last_name) = generate_name(nationality_id, genero, rng);
        let full_name = format!("{} {}", first_name, last_name);
        if !existing_names.contains(&full_name) {
            return (first_name, last_name);
        }
    }

    let pool = get_name_pool(nationality_id).unwrap_or(&NAME_POOLS[0]);
    let first_names = if genero.eq_ignore_ascii_case("F") && !pool.nomes_femininos.is_empty() {
        pool.nomes_femininos
    } else {
        pool.nomes_masculinos
    };

    for first_name in first_names {
        for last_name in pool.sobrenomes {
            let full_name = format!("{} {}", first_name, last_name);
            if !existing_names.contains(&full_name) {
                return ((*first_name).to_string(), (*last_name).to_string());
            }
        }
    }

    let base_first = first_names[0].to_string();
    let base_last = pool.sobrenomes[0];
    let mut suffix = 2_u32;
    loop {
        let forced_last = format!("{} {}", base_last, suffix);
        let full_name = format!("{} {}", base_first, forced_last);
        if !existing_names.contains(&full_name) {
            return (base_first.clone(), forced_last);
        }
        suffix += 1;
    }
}

pub fn random_gender(rng: &mut impl Rng) -> &'static str {
    if rng.gen_ratio(1, 20) {
        "F"
    } else {
        "M"
    }
}

pub fn generate_pilot_identity(
    existing_names: &HashSet<String>,
    rng: &mut impl Rng,
) -> PilotIdentity {
    let nationality = random_nationality(rng);
    let genero = random_gender(rng);
    let (primeiro_nome, sobrenome) =
        generate_unique_name(nationality.id, genero, existing_names, rng);
    let nome_completo = format!("{} {}", primeiro_nome, sobrenome);

    PilotIdentity {
        nome_completo,
        primeiro_nome,
        sobrenome,
        nacionalidade_id: nationality.id.to_string(),
        nacionalidade_label: format_nationality(nationality.id, genero, "pt-BR"),
        genero: genero.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rand::{rngs::StdRng, SeedableRng};

    use super::*;
    use crate::generators::nationality::get_all_nationalities;

    #[test]
    fn test_generate_name_returns_nonempty() {
        let mut rng = StdRng::seed_from_u64(11);
        let (first_name, last_name) = generate_name("br", "M", &mut rng);
        assert!(!first_name.is_empty());
        assert!(!last_name.is_empty());
    }

    #[test]
    fn test_generate_unique_name_no_collision() {
        let mut rng = StdRng::seed_from_u64(22);
        let mut existing = HashSet::new();

        for _ in 0..50 {
            let (first_name, last_name) = generate_unique_name("gb", "M", &existing, &mut rng);
            let full_name = format!("{} {}", first_name, last_name);
            assert!(existing.insert(full_name));
        }
    }

    #[test]
    fn test_random_gender_distribution() {
        let mut rng = StdRng::seed_from_u64(33);
        let mut female_count = 0;
        for _ in 0..1000 {
            if random_gender(&mut rng) == "F" {
                female_count += 1;
            }
        }

        assert!((20..=100).contains(&female_count));
    }

    #[test]
    fn test_generate_pilot_identity_complete() {
        let mut rng = StdRng::seed_from_u64(44);
        let existing = HashSet::new();
        let identity = generate_pilot_identity(&existing, &mut rng);

        assert!(!identity.nome_completo.is_empty());
        assert!(!identity.primeiro_nome.is_empty());
        assert!(!identity.sobrenome.is_empty());
        assert!(!identity.nacionalidade_id.is_empty());
        assert!(!identity.nacionalidade_label.is_empty());
        assert!(identity.genero == "M" || identity.genero == "F");
    }

    #[test]
    fn test_all_nationalities_have_name_pools() {
        for nationality in get_all_nationalities() {
            assert!(
                get_name_pool(nationality.id).is_some(),
                "missing pool for {}",
                nationality.id
            );
        }
    }

    #[test]
    fn test_name_pools_minimum_sizes() {
        for pool in get_all_name_pools() {
            assert!(pool.nomes_masculinos.len() >= 15, "{}", pool.nationality_id);
            assert!(pool.nomes_femininos.len() >= 4, "{}", pool.nationality_id);
            assert!(pool.sobrenomes.len() >= 15, "{}", pool.nationality_id);
        }
    }

    #[test]
    fn test_generate_200_unique_pilots() {
        let mut rng = StdRng::seed_from_u64(55);
        let mut existing = HashSet::new();

        for _ in 0..200 {
            let identity = generate_pilot_identity(&existing, &mut rng);
            assert!(existing.insert(identity.nome_completo));
        }
    }
}
