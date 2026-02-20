use super::error::SchedulerError;
use cis_traits::scheduler::{Dag, TaskNode};
use cis_types::{Task, TaskId};
use std::collections::{HashMap, HashSet, VecDeque};

pub struct TaskGraph {
    nodes: HashMap<TaskId, Task>,
    edges: Vec<(TaskId, TaskId)>,
}

impl TaskGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
        }
    }

    pub fn from_tasks(tasks: Vec<Task>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let mut graph = Self::new();
        for task in tasks {
            let task_id = task.id.clone();
            for dep in &task.dependencies {
                graph.edges.push((dep.clone(), task_id.clone()));
            }
            graph.nodes.insert(task_id, task);
        }
        Ok(graph)
    }

    pub fn from_dag(dag: &Dag) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let mut graph = Self::new();
        for node in &dag.tasks {
            for dep in &node.dependencies {
                graph.edges.push((dep.clone(), node.id.clone()));
            }
            graph.nodes.insert(node.id.clone(), node.task.clone());
        }
        Ok(graph)
    }

    pub fn add_task(&mut self, task: Task) -> Result<(), Box<dyn Error + Send + Sync>> {
        let task_id = task.id.clone();
        self.nodes.insert(task_id, task);
        Ok(())
    }

    pub fn add_dependency(
        &mut self,
        from: TaskId,
        to: TaskId,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if !self.nodes.contains_key(&from) {
            return Err(Box::new(SchedulerError::TaskNotFound(from)));
        }
        if !self.nodes.contains_key(&to) {
            return Err(Box::new(SchedulerError::TaskNotFound(to)));
        }
        self.edges.push((from, to));
        Ok(())
    }

    pub fn validate(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.topological_sort()?;
        Ok(())
    }

    pub fn topological_sort(&self) -> Result<Vec<TaskId>, Box<dyn Error + Send + Sync>> {
        let mut in_degree: HashMap<TaskId, usize> =
            self.nodes.keys().map(|k| (k.clone(), 0)).collect();

        for (_, to) in &self.edges {
            *in_degree.entry(to.clone()).or_insert(0) += 1;
        }

        let mut queue: VecDeque<TaskId> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(k, _)| k.clone())
            .collect();

        let mut result = Vec::new();

        while let Some(node) = queue.pop_front() {
            result.push(node.clone());

            for (from, to) in &self.edges {
                if *from == node {
                    if let Some(degree) = in_degree.get_mut(to) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(to.clone());
                        }
                    }
                }
            }
        }

        if result.len() != self.nodes.len() {
            return Err(Box::new(SchedulerError::CyclicDependency));
        }

        Ok(result)
    }

    pub fn into_nodes(self) -> Vec<TaskNode> {
        self.nodes
            .into_iter()
            .map(|(id, task)| TaskNode {
                id: id.clone(),
                task,
                dependencies: self
                    .edges
                    .iter()
                    .filter(|(_, to)| *to == id)
                    .map(|(from, _)| from.clone())
                    .collect(),
            })
            .collect()
    }

    pub fn into_edges(self) -> Vec<(TaskId, TaskId)> {
        self.edges
    }
}

impl Default for TaskGraph {
    fn default() -> Self {
        Self::new()
    }
}
