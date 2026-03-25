#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DATA_DIR="$ROOT/datasets/data"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

mkdir -p "$DATA_DIR"

need() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "error: required command not found: $1" >&2
        exit 1
    }
}

need curl
need unzip
need perl

fetch() {
    local url="$1"
    local out="$2"
    curl -fsSL "$url" -o "$out"
}

write_csv_from_zip() {
    local zip_path="$1"
    local member="$2"
    local header="$3"
    local output="$4"
    printf '%s\n' "$header" > "$output"
    unzip -p "$zip_path" "$member" | sed '/^[[:space:]]*$/d' >> "$output"
}

fetch 'https://archive.ics.uci.edu/static/public/53/iris.zip' "$TMP/iris.zip"
write_csv_from_zip \
    "$TMP/iris.zip" \
    'iris.data' \
    'sepal_length_cm,sepal_width_cm,petal_length_cm,petal_width_cm,species' \
    "$DATA_DIR/iris.csv"

fetch 'https://archive.ics.uci.edu/static/public/109/wine.zip' "$TMP/wine.zip"
write_csv_from_zip \
    "$TMP/wine.zip" \
    'wine.data' \
    'class,alcohol,malic_acid,ash,alcalinity_of_ash,magnesium,total_phenols,flavanoids,nonflavanoid_phenols,proanthocyanins,color_intensity,hue,od280_od315,proline' \
    "$DATA_DIR/wine.csv"

fetch 'https://archive.ics.uci.edu/static/public/9/auto%2Bmpg.zip' "$TMP/auto_mpg.zip"
printf '%s\n' 'mpg,cylinders,displacement,horsepower,weight,acceleration,model_year,origin,car_name' > "$DATA_DIR/auto_mpg.csv"
unzip -p "$TMP/auto_mpg.zip" 'auto-mpg.data' | perl -ne '
    next if /^\s*$/;
    s/^\s+//;
    s/\s+$//;
    if (/^(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+"(.*)"$/) {
        my @fields = ($1, $2, $3, $4, $5, $6, $7, $8);
        my $name = $9;
        $name =~ s/"/""/g;
        print join(",", @fields, qq{"$name"}), "\n";
    } else {
        die "unparsed auto-mpg row: $_";
    }
' >> "$DATA_DIR/auto_mpg.csv"

fetch 'https://archive.ics.uci.edu/static/public/1/abalone.zip' "$TMP/abalone.zip"
write_csv_from_zip \
    "$TMP/abalone.zip" \
    'abalone.data' \
    'sex,length_mm,diameter_mm,height_mm,whole_weight_g,shucked_weight_g,viscera_weight_g,shell_weight_g,rings' \
    "$DATA_DIR/abalone.csv"

fetch 'https://archive.ics.uci.edu/static/public/17/breast%2Bcancer%2Bwisconsin%2Bdiagnostic.zip' "$TMP/wdbc.zip"
write_csv_from_zip \
    "$TMP/wdbc.zip" \
    'wdbc.data' \
    'id,diagnosis,radius_mean,texture_mean,perimeter_mean,area_mean,smoothness_mean,compactness_mean,concavity_mean,concave_points_mean,symmetry_mean,fractal_dimension_mean,radius_se,texture_se,perimeter_se,area_se,smoothness_se,compactness_se,concavity_se,concave_points_se,symmetry_se,fractal_dimension_se,radius_worst,texture_worst,perimeter_worst,area_worst,smoothness_worst,compactness_worst,concavity_worst,concave_points_worst,symmetry_worst,fractal_dimension_worst' \
    "$DATA_DIR/breast_cancer_wisconsin_diagnostic.csv"

fetch 'https://raw.githubusercontent.com/allisonhorst/palmerpenguins/master/inst/extdata/penguins.csv' "$DATA_DIR/penguins.csv"

printf 'Fetched %s datasets into %s\n' '6' "$DATA_DIR"
