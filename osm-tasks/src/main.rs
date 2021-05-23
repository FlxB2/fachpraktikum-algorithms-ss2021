extern crate core;

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::iter;
use std::iter::FromIterator;
use std::slice::Iter;
use std::time::Instant;

use osmpbf::{Element, ElementReader};
use rand::distributions::{Distribution, Uniform};
use rayon::prelude::*;

use crate::json_generator::JsonBuilder;
use crate::grid_graph::GridGraph;
use crate::kml_exporter::KML_export;
use crate::polygon_test::PointInPolygonTest;

mod grid_graph;
mod json_generator;
mod dijkstra;
mod kml_exporter;
mod polygon_test;


fn main() {
    //read_file("./monaco-latest.osm.pbf");
    //read_file("./iceland-coastlines.osm.pbf");
    //read_file("./planet-coastlines.osm.pbf");

    let mut kml = KML_export::init();
    //points_in_polygon.into_iter().for_each(|p| { kml.add_point(p, None) });
    let graph = GridGraph::new();

    for n in 0..graph.edges.len() {
        let e = graph.edges[n];
        kml.add_linestring( Vec::from([
            (graph.nodes[e.source].lat, graph.nodes[e.source].lon),
            (graph.nodes[e.target].lat, graph.nodes[e.target].lon)]), Some(e.source.to_string()));
    }
    graph.nodes.into_iter().for_each(|n| { kml.add_point((n.lat, n.lon), None) });
    kml.write_file("kml.kml".parse().unwrap());
}

fn read_file(path: &str) {
    let start_time = Instant::now();
    let reader = ElementReader::from_path(path).expect(&*format!("failed to read file {}", path));

    // key is the first node of the way; value is a tuple containing the last node and the whole way
    let mut coastlines: HashMap<i64, (i64, Vec<i64>)> = HashMap::new();
    let mut node_to_location: HashMap<i64, (f64, f64)> = HashMap::new();
    println!("Reading file {}", path);

    /**
     Assumptions:
     - each coastline way ends with a node which is contained in another coastline way
    **/
    reader.for_each(|item| {
        match item {
            Element::Way(way) => {
                if let Some(_) = way.tags().find(|(k, v)| *k == "natural" && *v == "coastline") {
                    let first_node_id = way.refs().next().expect("way does not contain any nodes");
                    if let Some(last) = way.refs().last() {
                        coastlines.insert(first_node_id, (last, way.refs().collect()));
                    }
                }
            }
            Element::Node(node) => {
                node_to_location.insert(node.id(), (node.lon(), node.lat()));
            }
            Element::DenseNode(node) => {
                node_to_location.insert(node.id(), (node.lon(), node.lat()));
            }
            _ => {}
        }
    });
    println!("Reading done in {} sec", start_time.elapsed().as_secs());
    let merge_start_time = Instant::now();
    let mut polygons: Vec<Vec<(f64, f64)>> = merge_ways_to_polygons1(coastlines, node_to_location);

    println!("Merged polygons coastlines to {} polygons in {} sec", polygons.len(), merge_start_time.elapsed().as_secs());
    check_polygons_closed(&polygons);

    // sort polygons by size so that we check the bigger before the smaller ones
    polygons.sort_by(|a, b| b.len().cmp(&a.len()));
    println!("Number of Polygons: {}", polygons.first().unwrap().len());

    /*
    let file = "poly";
    JsonBuilder::new(String::from(file)).add_polygons(polygons).build();
    println!("Generated json");*/

    /*let point_test = PointInPolygonTest::new(vec![polygons[3].clone()]);
    let lon_min = -20.342559814453125;
    let lon_max = -20.20832061767578;
    let lat_min = 63.39413573718524;
    let lat_max = 63.45864118848073;

    let points_in_polygon = test_random_points_in_polygon(&point_test, 10000, (lon_min, lon_max, lat_min, lat_max)); */
    //write_to_file("island".parse().unwrap(), points_to_json(points_in_polygon));
    let mut kml = KML_export::init();
    //points_in_polygon.into_iter().for_each(|p| { kml.add_point(p, None) });
    let graph = GridGraph::new();
    //graph.nodes.into_iter().foreach(|n| { kml.add_point(n, None) });
    kml.write_file("kml.kml".parse().unwrap());
}

fn write_to_file(name: String, data: String) {
    let mut file = File::create(name).expect("Could not open file");
    file.write_all(data.as_ref()).expect("Could not write file");
}

fn points_to_json(points: Vec<(f64, f64)>) -> String {
    let points_string = format!("{:?}", points).replace("(", "[").replace(")", "]\n");
    let feature = format!("{{ \"type\": \"MultiPoint\",
    \"coordinates\": {}
}}", points_string);
    format!("{{
  \"type\": \"FeatureCollection\",
  \"features\": [
    {{
      \"type\": \"Feature\",
      \"properties\": {{}},
      \"geometry\":  {} \
    }}
  ]
}}", feature)
}

fn lines_to_json(lines: Vec<((f64, f64), (f64, f64))>) -> String {
    let mut features = String::new();
    for line in lines {
        let geometry = format!("{{ \"type\": \"LineString\",
    \"coordinates\": [[{},{}],[{},{}]]
}}", line.0.0, line.0.1, line.1.0, line.1.1);
        let feature = format!("{{
      \"type\": \"Feature\",
      \"properties\": {{}},
      \"geometry\":  {} \
    }}\n,", geometry);
        features = features + &*feature;
    }
    features.pop();
    format!("{{
  \"type\": \"FeatureCollection\",
  \"features\": [
    {}
  ]
}}", features)
}

fn export_polygons_with_resolution(polygons: Vec<Vec<(f64,f64)>>, path: String, max_nodes_per_polygon : usize) {
    let mut kml = KML_export::init();
    polygons.into_iter().map(|poly| {
        if poly.len() > max_nodes_per_polygon {
            let inverse_factor = poly.len()/max_nodes_per_polygon;
            let first = *poly.first().unwrap();
            return vec![first].into_iter().chain(poly.into_iter().step_by(inverse_factor)).collect();
        }
        poly
    }).for_each(|poly| {
        kml.add_polygon(poly, None);
    });
    kml.write_file(path);
}

fn test_random_points_in_polygon(polygon_test: &PointInPolygonTest, number_of_points_to_test: usize, (lon_min, lon_max, lat_min, lat_max): (f64, f64, f64, f64)) -> Vec<(f64, f64)> {
    let mut rng = rand::thread_rng();
    let rng_lat = Uniform::from(lat_min..lat_max);
    let rng_lon = Uniform::from(lon_min..lon_max);
    let coords: Vec<(f64, f64)> = iter::repeat(0).take(number_of_points_to_test).map(|_| {
        (rng_lon.sample(&mut rng), rng_lat.sample(&mut rng))
    }).collect();
    coords.into_par_iter().map(|test_point: (f64, f64)| {
        if polygon_test.check_intersection(test_point.clone()) {
            return test_point;
        }
        return (f64::NAN, f64::NAN);
    }).filter(|(lon, lat): &(f64, f64)| { !lon.is_nan() }).collect()
}

fn merge_ways_to_polygons1(coastlines: HashMap<i64, (i64, Vec<i64>)>, node_to_location: HashMap<i64, (f64, f64)>) -> Vec<Vec<(f64, f64)>> {
    let mut polygons: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut visited: HashMap<i64, bool> = HashMap::new();
    for key in coastlines.keys() {
        visited.insert(*key, false);
    }

    for key in coastlines.keys() {
        if *visited.get(key).unwrap_or(&true) {
            continue;
        }
        let mut start = key;
        let mut poly: Vec<(f64, f64)> = vec![*node_to_location.get(start).expect("Could not find coords for start node")];

        loop {
            if let Some((end, way)) = coastlines.get(start) {
                // add way to polygon
                for node in way[1..].iter() {
                    if let Some((lat, lon)) = node_to_location.get(node) {
                        poly.push((*lat, *lon));
                    } else {
                        print!("could not find coords for node {}", node)
                    }
                }
                visited.insert(*start, true);
                start = end;
            } else {
                println!("Could not find node {} in coastlines map", start);
                break;
            }
            if let Some(visit) = visited.get(start) {
                if *visit == true {
                    polygons.push(poly);
                    break;
                }
            } else {
                println!("Could not find node {} in visited map", start);
            }
        }
    }
    return polygons;
}

#[allow(dead_code)]
fn merge_ways_to_polygons2(coastlines: HashMap<i64, (i64, Vec<i64>)>, node_to_location: HashMap<i64, (f64, f64)>) -> Vec<Vec<(f64, f64)>> {
    let mut unprocessed_coastlines: HashSet<&i64> = HashSet::from_iter(coastlines.keys());
    let mut polygons: Vec<Vec<(f64, f64)>> = vec![];

    while !unprocessed_coastlines.is_empty() {
        let first_node = **unprocessed_coastlines.iter().next().expect("Coastline already processed");
        let (mut next_node, nodes) = coastlines.get(&first_node).expect("coastline not found in map");
        unprocessed_coastlines.remove(&first_node);

        let mut polygon: Vec<(f64, f64)> = Vec::with_capacity(nodes.len());
        append_coords_from_map_for_nodes(&node_to_location, &mut polygon, &mut nodes.iter());
        while next_node != first_node {
            unprocessed_coastlines.remove(&next_node);
            reserve_space_if_below_threshold(&mut polygon, 2000, 5000);
            if let Some((next_next_node, nodes)) = coastlines.get(&next_node) {
                append_coords_from_map_for_nodes(&node_to_location, &mut polygon, &mut nodes[1..].iter());
                next_node = *next_next_node;
            } else {
                println!("Could not find next node {}", next_node);
                break;
            }
        }
        polygon.shrink_to_fit();
        reserve_space_if_below_threshold(&mut polygons, 1, 5000);
        polygons.push(polygon);
    }
    polygons.shrink_to_fit();
    return polygons;
}

#[inline]
fn append_coords_from_map_for_nodes(node_to_location: &HashMap<i64, (f64, f64)>, polygon: &mut Vec<(f64, f64)>, nodes: &mut Iter<i64>) {
    nodes.for_each(|node_id| {
        if let Some(coord) = node_to_location.get(node_id) {
            polygon.push(*coord);
        } else {
            //Should not happen
            println!("Could not resolve coord for node: {}", node_id)
        }
    });
}

#[inline]
// use this function to check periodically if the capacity of a vector is below a limit
// to avoid expensive memory allocation at every insert operation
fn reserve_space_if_below_threshold<T>(vector: &mut Vec<T>, minimum_size: usize, reserved_size: usize) {
    if vector.capacity() < minimum_size {
        vector.reserve(reserved_size);
    }
}

fn check_polygons_closed(polygons: &Vec<Vec<(f64, f64)>>) -> bool {
    let polygon_count = polygons.len();
    let closed_polygons_count = polygons.iter().filter(|polygon| !polygon.is_empty() && polygon.first() == polygon.last()).count();
    println!("{} of {} polygons are closed", closed_polygons_count, polygon_count);
    polygon_count == closed_polygons_count
}


