//! Uses the linked list as the queue used in the shortest path graph algorithm.

extern crate linked_list;
extern crate rand;

use linked_list::LinkedList;
use std::collections::VecDeque;
use rand::prelude::*;

struct Graph {
    adj_list: Vec<Vec<usize>>
}

impl Graph {
    fn size(&self) -> usize {
        self.adj_list.len()
    }
    fn connect(&mut self, a: usize, b: usize) {
        if a != b {
            self.adj_list[a].push(b);
            self.adj_list[b].push(a);
        }
    }
    fn neighbours<'a>(&'a self, node: usize) -> impl Iterator<Item = usize> + 'a {
        self.adj_list[node].iter().cloned()
    }
}

fn random_graph() -> Graph {
    let mut rng = thread_rng();
    let mut graph = Graph { adj_list: vec![Vec::new(); 128] };
    for _ in 0..256 {
        let a = rng.gen_range(0, 128);
        let b = rng.gen_range(0, 128);
        graph.connect(a, b);
    }
    graph
}

#[test]
fn shortest_path() {
    let graph = random_graph();

    // find the shortest path from node 0 to 1
    let start = 0;
    let end = 1;

    // we perform the algorithm with two queues simultaneously, checking that they do the
    // same thing
    let mut list_queue = LinkedList::new();
    let mut vec_queue = VecDeque::new();

    let mut visited = vec![false; graph.size()];

    list_queue.push_back((start, 0));
    vec_queue.push_back((start, 0));

    while !list_queue.is_empty() {
        assert!(!vec_queue.is_empty());
        assert_eq!(vec_queue.len(), list_queue.len());

        let (node, len) = list_queue.pop_front().unwrap();
        assert_eq!((node, len), vec_queue.pop_front().unwrap());

        if node == end {
            println!("shortest path has length {}", len);
            return;
        }
        if visited[node] {
            continue;
        }
        visited[node] = true;

        for neighbour in graph.neighbours(node) {
            if !visited[neighbour] {
                list_queue.push_back((neighbour, len + 1));
                vec_queue.push_back((neighbour, len + 1));
            }
        }
    }
    assert!(vec_queue.is_empty());
    println!("no path");
}
