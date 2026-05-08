#![allow(dead_code)]

pub struct CarInfo {
    pub nome: &'static str,
    pub car_path: &'static str,
    pub car_id: u32,
    pub car_class_id: u32,
    pub categorias: &'static [&'static str],
    pub classe: &'static str,
    pub marca: &'static str,
}

pub type CarDefinition = CarInfo;

static MAZDA_CATS: [&str; 3] = ["mazda_rookie", "mazda_amador", "production_challenger"];
static TOYOTA_CATS: [&str; 3] = ["toyota_rookie", "toyota_amador", "production_challenger"];
static BMW_M2_CATS: [&str; 2] = ["bmw_m2", "production_challenger"];
static GT4_CATS: [&str; 2] = ["gt4", "endurance"];
static GT3_CATS: [&str; 2] = ["gt3", "endurance"];
static LMP2_CATS: [&str; 2] = ["lmp2", "endurance"];

static CARS: &[CarInfo] = &[
    CarInfo {
        nome: "Mazda MX-5 2016",
        car_path: "mx52016",
        car_id: 67,
        car_class_id: 3011,
        categorias: &MAZDA_CATS,
        classe: "monomarca",
        marca: "Mazda",
    },
    CarInfo {
        nome: "Toyota GR86",
        car_path: "toyotagr86",
        car_id: 154,
        car_class_id: 3012,
        categorias: &TOYOTA_CATS,
        classe: "monomarca",
        marca: "Toyota",
    },
    CarInfo {
        nome: "BMW M2 CS Racing",
        car_path: "bmwm2csracing",
        car_id: 134,
        car_class_id: 3013,
        categorias: &BMW_M2_CATS,
        classe: "monomarca",
        marca: "BMW",
    },
    CarInfo {
        nome: "BMW M4 GT4",
        car_path: "bmwm4gt4",
        car_id: 120,
        car_class_id: 4001,
        categorias: &GT4_CATS,
        classe: "gt4",
        marca: "BMW",
    },
    CarInfo {
        nome: "Porsche 718 Cayman GT4",
        car_path: "porsche718gt4",
        car_id: 121,
        car_class_id: 4001,
        categorias: &GT4_CATS,
        classe: "gt4",
        marca: "Porsche",
    },
    CarInfo {
        nome: "Mercedes-AMG GT4",
        car_path: "mercedesamggt4",
        car_id: 122,
        car_class_id: 4001,
        categorias: &GT4_CATS,
        classe: "gt4",
        marca: "Mercedes-AMG",
    },
    CarInfo {
        nome: "Aston Martin Vantage GT4",
        car_path: "astonmartinvantagt4",
        car_id: 123,
        car_class_id: 4001,
        categorias: &GT4_CATS,
        classe: "gt4",
        marca: "Aston Martin",
    },
    CarInfo {
        nome: "McLaren 570S GT4",
        car_path: "mclaren570sgt4",
        car_id: 124,
        car_class_id: 4001,
        categorias: &GT4_CATS,
        classe: "gt4",
        marca: "McLaren",
    },
    CarInfo {
        nome: "Toyota GR Supra GT4",
        car_path: "toyotagrsupraegt4",
        car_id: 125,
        car_class_id: 4001,
        categorias: &GT4_CATS,
        classe: "gt4",
        marca: "Toyota",
    },
    CarInfo {
        nome: "Ferrari 296 GT3",
        car_path: "ferrari296gt3",
        car_id: 201,
        car_class_id: 5001,
        categorias: &GT3_CATS,
        classe: "gt3",
        marca: "Ferrari",
    },
    CarInfo {
        nome: "BMW M4 GT3",
        car_path: "bmwm4gt3",
        car_id: 202,
        car_class_id: 5001,
        categorias: &GT3_CATS,
        classe: "gt3",
        marca: "BMW",
    },
    CarInfo {
        nome: "Mercedes-AMG GT3",
        car_path: "mercedesamggt3",
        car_id: 203,
        car_class_id: 5001,
        categorias: &GT3_CATS,
        classe: "gt3",
        marca: "Mercedes-AMG",
    },
    CarInfo {
        nome: "Porsche 911 GT3 R (992)",
        car_path: "porsche992gt3r",
        car_id: 204,
        car_class_id: 5001,
        categorias: &GT3_CATS,
        classe: "gt3",
        marca: "Porsche",
    },
    CarInfo {
        nome: "Lamborghini Huracan GT3 EVO",
        car_path: "lamborghinihuracangt3evo",
        car_id: 205,
        car_class_id: 5001,
        categorias: &GT3_CATS,
        classe: "gt3",
        marca: "Lamborghini",
    },
    CarInfo {
        nome: "Aston Martin Vantage GT3",
        car_path: "astonmartinvantagegt3",
        car_id: 206,
        car_class_id: 5001,
        categorias: &GT3_CATS,
        classe: "gt3",
        marca: "Aston Martin",
    },
    CarInfo {
        nome: "McLaren 720S GT3 EVO",
        car_path: "mclaren720sgt3evo",
        car_id: 207,
        car_class_id: 5001,
        categorias: &GT3_CATS,
        classe: "gt3",
        marca: "McLaren",
    },
    CarInfo {
        nome: "Audi R8 LMS Evo II GT3",
        car_path: "audir8lmsevoii",
        car_id: 208,
        car_class_id: 5001,
        categorias: &GT3_CATS,
        classe: "gt3",
        marca: "Audi",
    },
    CarInfo {
        nome: "Chevrolet Corvette Z06 GT3.R",
        car_path: "chevroletcorvettezt06gt3r",
        car_id: 209,
        car_class_id: 5001,
        categorias: &GT3_CATS,
        classe: "gt3",
        marca: "Chevrolet",
    },
    CarInfo {
        nome: "Ford Mustang GT3",
        car_path: "fordmustanggt3",
        car_id: 210,
        car_class_id: 5001,
        categorias: &GT3_CATS,
        classe: "gt3",
        marca: "Ford",
    },
    CarInfo {
        nome: "Acura NSX GT3 Evo 22",
        car_path: "acuransxgt3evo22",
        car_id: 211,
        car_class_id: 5001,
        categorias: &GT3_CATS,
        classe: "gt3",
        marca: "Acura",
    },
    CarInfo {
        nome: "Dallara P217 LMP2",
        car_path: "dallarap217lmp2",
        car_id: 301,
        car_class_id: 6001,
        categorias: &LMP2_CATS,
        classe: "lmp2",
        marca: "Dallara",
    },
];

pub fn get_car(car_path: &str) -> Option<&'static CarInfo> {
    CARS.iter().find(|car| car.car_path == car_path)
}

pub fn get_all_cars() -> &'static [CarInfo] {
    CARS
}

pub fn get_cars_for_category(category_id: &str) -> Vec<&'static CarInfo> {
    CARS.iter()
        .filter(|car| car.categorias.contains(&category_id))
        .collect()
}

pub fn get_cars_by_class(classe: &str) -> Vec<&'static CarInfo> {
    CARS.iter().filter(|car| car.classe == classe).collect()
}

pub fn get_cars_by_brand(marca: &str) -> Vec<&'static CarInfo> {
    CARS.iter().filter(|car| car.marca == marca).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cars_for_endurance_includes_all_gt4() {
        let cars = get_cars_for_category("endurance");
        let gt4_count = cars.iter().filter(|car| car.classe == "gt4").count();
        assert!(gt4_count >= 6);
    }
}
