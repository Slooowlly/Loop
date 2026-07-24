//! Listas de primeiros nomes e sobrenomes das nacionalidades europeias.

pub(crate) static GB_MALE: &[&str] = &[
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
pub(crate) static GB_FEMALE: &[&str] = &[
    "Emily",
    "Charlotte",
    "Sophie",
    "Hannah",
    "Jessica",
    "Olivia",
    "Grace",
    "Amelia",
];
pub(crate) static GB_LAST: &[&str] = &[
    "Smith", "Jones", "Williams", "Brown", "Taylor", "Wilson", "Davies", "Evans", "Thomas",
    "Roberts", "Walker", "Wright", "Turner", "Hill", "Clarke", "Mitchell", "Cooper", "Ward",
    "Morris", "King", "Green", "Baker", "Hall", "Wood", "Harris", "Clark", "Harrison", "Scott",
    "Edwards", "Murray",
];

pub(crate) static DE_MALE: &[&str] = &[
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
pub(crate) static DE_FEMALE: &[&str] = &[
    "Anna", "Laura", "Sophie", "Leonie", "Lisa", "Marie", "Julia", "Lena",
];
pub(crate) static DE_LAST: &[&str] = &[
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

pub(crate) static FR_MALE: &[&str] = &[
    "Lucas", "Nathan", "Jules", "Louis", "Hugo", "Theo", "Antoine", "Maxime", "Adrien", "Clement",
    "Matthieu", "Julien", "Bastien", "Remy", "Alexis", "Nicolas", "Gabriel", "Romain", "Quentin",
    "Vincent", "Benoit", "Damien", "Thomas", "Arthur", "Martin",
];
pub(crate) static FR_FEMALE: &[&str] = &["Camille", "Emma", "Chloe", "Lucie", "Manon", "Lea"];
pub(crate) static FR_LAST: &[&str] = &[
    "Martin", "Bernard", "Thomas", "Petit", "Robert", "Richard", "Durand", "Dubois", "Moreau",
    "Laurent", "Simon", "Michel", "Lefebvre", "Leroy", "Roux", "David", "Bertrand", "Morel",
    "Fournier", "Girard", "Andre", "Mercier", "Dupont", "Lambert", "Bonnet",
];

pub(crate) static IT_MALE: &[&str] = &[
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
pub(crate) static IT_FEMALE: &[&str] =
    &["Giulia", "Chiara", "Martina", "Sara", "Elena", "Francesca"];
pub(crate) static IT_LAST: &[&str] = &[
    "Villa", "Russo", "Ferraro", "Esposito", "Bianchi", "Romano", "Colombo", "Ricci", "Marino",
    "Greco", "Bruno", "Gallo", "Conti", "DeLuca", "Mancini", "Costa", "Giordano", "Rinaldi",
    "Lombardi", "Moretti", "Barbieri", "Fontana", "Caruso", "Leone", "Santoro",
];

pub(crate) static ES_MALE: &[&str] = &[
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
pub(crate) static ES_FEMALE: &[&str] = &["Lucia", "Marta", "Elena", "Paula", "Irene", "Carmen"];
pub(crate) static ES_LAST: &[&str] = &[
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

pub(crate) static NL_MALE: &[&str] = &[
    "Daan", "Milan", "Sem", "Luuk", "Bram", "Jesse", "Stijn", "Niels", "Thijs", "Joris", "Tom",
    "Koen", "Sven", "Ruben", "Lars", "Pieter", "Willem", "Timo", "Bas", "Cas",
];
pub(crate) static NL_FEMALE: &[&str] = &["Emma", "Sanne", "Lisa", "Noa", "Julia"];
pub(crate) static NL_LAST: &[&str] = &[
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

pub(crate) static FI_MALE: &[&str] = &[
    "Mikael", "Joonas", "Antti", "Aleksi", "Eetu", "Oskari", "Ville", "Juho", "Mikko", "Sami",
    "Toni", "Jesse", "Lauri", "Arttu", "Petri",
];
pub(crate) static FI_FEMALE: &[&str] = &["Aino", "Emilia", "Laura", "Sanni"];
pub(crate) static FI_LAST: &[&str] = &[
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

pub(crate) static BE_MALE: &[&str] = &[
    "Arthur", "Louis", "Maxime", "Julien", "Thomas", "Nicolas", "Benoit", "Antoine", "Cyril",
    "David", "Hugo", "Matthias", "Simon", "Cedric", "Victor",
];
pub(crate) static BE_FEMALE: &[&str] = &["Elise", "Julie", "Laura", "Manon"];
pub(crate) static BE_LAST: &[&str] = &[
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

pub(crate) static PT_MALE: &[&str] = &[
    "Joao", "Tiago", "Diogo", "Rui", "Miguel", "Andre", "Nuno", "Pedro", "Bruno", "Goncalo",
    "Tomas", "Afonso", "Ricardo", "Hugo", "Vasco",
];
pub(crate) static PT_FEMALE: &[&str] = &["Ines", "Marta", "Beatriz", "Joana"];
pub(crate) static PT_LAST: &[&str] = &[
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

pub(crate) static AT_MALE: &[&str] = &[
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
pub(crate) static AT_FEMALE: &[&str] = &["Anna", "Lisa", "Julia", "Sarah"];
pub(crate) static AT_LAST: &[&str] = &[
    "Gruber", "Huber", "Wagner", "Pichler", "Moser", "Steiner", "Mayer", "Seidl", "Hofer", "Bauer",
    "Eder", "Fuchs", "Leitner", "Winter", "Schmid",
];

pub(crate) static CH_MALE: &[&str] = &[
    "Luca", "Jan", "Noah", "Simon", "Matthias", "David", "Pascal", "Fabian", "Joel", "Timo",
    "Nils", "Marco", "Jonas", "Cedric", "Adrian",
];
pub(crate) static CH_FEMALE: &[&str] = &["Lara", "Nina", "Lea", "Julia"];
pub(crate) static CH_LAST: &[&str] = &[
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

pub(crate) static DK_MALE: &[&str] = &[
    "Mads", "Jonas", "Lasse", "Emil", "Frederik", "Rasmus", "Nikolaj", "Anders", "Kasper",
    "Mikkel", "Oliver", "Troels", "Mathias", "Jakob", "Soren",
];
pub(crate) static DK_FEMALE: &[&str] = &["Emma", "Clara", "Sofie", "Freja"];
pub(crate) static DK_LAST: &[&str] = &[
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

pub(crate) static SE_MALE: &[&str] = &[
    "Erik", "Viktor", "Anton", "Filip", "Emil", "Oskar", "Johan", "Henrik", "Ludvig", "Axel",
    "Albin", "Robin", "Marcus", "Gustav", "Simon",
];
pub(crate) static SE_FEMALE: &[&str] = &["Elsa", "Maja", "Alva", "Julia"];
pub(crate) static SE_LAST: &[&str] = &[
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

pub(crate) static NO_MALE: &[&str] = &[
    "Ola", "Jon", "Lars", "Magnus", "Andreas", "Emil", "Kristian", "Tobias", "Sindre", "Marius",
    "Henrik", "Vetle", "Eirik", "Martin", "Fredrik",
];
pub(crate) static NO_FEMALE: &[&str] = &["Ingrid", "Nora", "Emma", "Sara"];
pub(crate) static NO_LAST: &[&str] = &[
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

pub(crate) static PL_MALE: &[&str] = &[
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
pub(crate) static PL_FEMALE: &[&str] = &["Anna", "Katarzyna", "Magdalena", "Oliwia"];
pub(crate) static PL_LAST: &[&str] = &[
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
