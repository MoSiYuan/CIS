//! # Skill Chain
//!
//! 技能链编排系统，支持管道式的技能组合。
//!
//! ## 功能特性
//!
//! - 链式技能执行
//! - 输入输出映射
//! - 条件执行
//! - 错误处理

use std::path::PathBuf;

use serde_json::Value;

use crate::error::Result;

/// 技能链
pub struct SkillChain {
    /// 执行步骤
    steps: Vec<ChainStep>,
    /// 执行上下文
    context: ChainContext,
}

/// 链步骤
pub struct ChainStep {
    /// 技能ID
    pub skill_id: String,
    /// 输入映射 (from_output, to_param)
    pub input_mapping: Vec<(String, String)>,
    /// 执行条件
    pub condition: Option<String>,
}

/// 链上下文
pub struct ChainContext {
    /// 初始输入
    pub initial_input: Value,
    /// 中间结果
    pub intermediate_results: Vec<StepResult>,
    /// 项目路径
    pub project_path: Option<PathBuf>,
}

/// 步骤执行结果
#[derive(Debug, Clone)]
pub struct StepResult {
    /// 步骤索引
    pub step_index: usize,
    /// 技能ID
    pub skill_id: String,
    /// 输出
    pub output: Value,
    /// 是否成功
    pub success: bool,
}

impl SkillChain {
    /// 创建链构建器
    pub fn builder() -> ChainBuilder {
        ChainBuilder::new()
    }

    /// 创建新的技能链
    pub fn new(initial_input: Value) -> Self {
        Self {
            steps: Vec::new(),
            context: ChainContext {
                initial_input,
                intermediate_results: Vec::new(),
                project_path: None,
            },
        }
    }

    /// 添加步骤
    pub fn add_step(&mut self, skill_id: String) -> &mut Self {
        self.steps.push(ChainStep {
            skill_id,
            input_mapping: Vec::new(),
            condition: None,
        });
        self
    }

    /// 执行链
    pub async fn execute<F, Fut>(&mut self, executor: F) -> Result<Vec<StepResult>>
    where
        F: Fn(&str, Value) -> Fut,
        Fut: std::future::Future<Output = Result<Value>>,
    {
        for (idx, step) in self.steps.iter().enumerate() {
            // 准备输入
            let input = self.prepare_input(step);

            // 检查条件
            if let Some(cond) = &step.condition {
                if !self.evaluate_condition(cond) {
                    continue;
                }
            }

            // 执行
            match executor(&step.skill_id, input).await {
                Ok(output) => {
                    self.context.intermediate_results.push(StepResult {
                        step_index: idx,
                        skill_id: step.skill_id.clone(),
                        output,
                        success: true,
                    });
                }
                Err(e) => {
                    self.context.intermediate_results.push(StepResult {
                        step_index: idx,
                        skill_id: step.skill_id.clone(),
                        output: Value::String(e.to_string()),
                        success: false,
                    });
                    // 链失败处理：可以继续或中断
                }
            }
        }

        Ok(self.context.intermediate_results.clone())
    }

    /// 同步执行链（用于非异步上下文）
    pub fn execute_sync<F>(&mut self, executor: F) -> Result<Vec<StepResult>>
    where
        F: Fn(&str, Value) -> Result<Value>,
    {
        for (idx, step) in self.steps.iter().enumerate() {
            // 准备输入
            let input = self.prepare_input(step);

            // 检查条件
            if let Some(cond) = &step.condition {
                if !self.evaluate_condition(cond) {
                    continue;
                }
            }

            // 执行
            match executor(&step.skill_id, input) {
                Ok(output) => {
                    self.context.intermediate_results.push(StepResult {
                        step_index: idx,
                        skill_id: step.skill_id.clone(),
                        output,
                        success: true,
                    });
                }
                Err(e) => {
                    self.context.intermediate_results.push(StepResult {
                        step_index: idx,
                        skill_id: step.skill_id.clone(),
                        output: Value::String(e.to_string()),
                        success: false,
                    });
                }
            }
        }

        Ok(self.context.intermediate_results.clone())
    }

    /// 准备步骤输入
    fn prepare_input(&self, step: &ChainStep) -> Value {
        let mut input = serde_json::Map::new();

        // 映射前序步骤的输出
        for (from_output, to_param) in &step.input_mapping {
            if let Some(prev_result) = self.context.intermediate_results.last() {
                if let Some(value) = prev_result.output.get(from_output) {
                    input.insert(to_param.clone(), value.clone());
                }
            }
        }

        // 如果没有映射，使用初始输入
        if input.is_empty() {
            return self.context.initial_input.clone();
        }

        serde_json::Value::Object(input)
    }

    /// 评估条件
    fn evaluate_condition(&self, _condition: &str) -> bool {
        // 简化实现：始终执行
        true
    }

    /// 获取执行结果
    pub fn results(&self) -> &[StepResult] {
        &self.context.intermediate_results
    }

    /// 获取最后一步结果
    pub fn last_result(&self) -> Option<&StepResult> {
        self.context.intermediate_results.last()
    }

    /// 设置项目路径
    pub fn with_project_path(mut self, path: PathBuf) -> Self {
        self.context.project_path = Some(path);
        self
    }
}

/// 链构建器
pub struct ChainBuilder {
    steps: Vec<ChainStep>,
}

impl ChainBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// 添加步骤
    pub fn then(mut self, skill_id: &str) -> Self {
        self.steps.push(ChainStep {
            skill_id: skill_id.to_string(),
            input_mapping: Vec::new(),
            condition: None,
        });
        self
    }

    /// 添加上一步到当前步骤的管道映射
    pub fn pipe(mut self, from_output: &str, to_param: &str) -> Self {
        if let Some(step) = self.steps.last_mut() {
            step.input_mapping.push((from_output.to_string(), to_param.to_string()));
        }
        self
    }

    /// 添加条件
    pub fn when(mut self, condition: &str) -> Self {
        if let Some(step) = self.steps.last_mut() {
            step.condition = Some(condition.to_string());
        }
        self
    }

    /// 构建链
    pub fn build(self, initial_input: Value) -> SkillChain {
        SkillChain {
            steps: self.steps,
            context: ChainContext {
                initial_input,
                intermediate_results: Vec::new(),
                project_path: None,
            },
        }
    }
}

impl Default for ChainBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 预定义技能链模板
pub struct ChainTemplates;

impl ChainTemplates {
    /// 分析-提交链
    pub fn analyze_and_commit(input: Value) -> SkillChain {
        SkillChain::builder()
            .then("cis-local:read")
            .then("cis-local:analyze")
            .pipe("analysis", "input")
            .then("cis-local:commit")
            .build(input)
    }

    /// 文件列表-分析链
    pub fn list_and_analyze(input: Value) -> SkillChain {
        SkillChain::builder()
            .then("cis-local:file-list")
            .pipe("files", "file_list")
            .then("cis-local:analyze")
            .build(input)
    }

    /// 完整工作流链
    pub fn full_workflow(input: Value) -> SkillChain {
        SkillChain::builder()
            .then("cis-local:file-list")
            .then("cis-local:read")
            .pipe("content", "input")
            .then("cis-local:analyze")
            .pipe("analysis", "input")
            .then("cis-local:commit")
            .build(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_builder() {
        let chain = SkillChain::builder()
            .then("skill1")
            .then("skill2")
            .pipe("output1", "input1")
            .build(serde_json::json!({"initial": true}));

        assert_eq!(chain.results().len(), 0);
    }

    #[test]
    fn test_chain_templates() {
        let input = serde_json::json!({"path": "/tmp"});
        
        let chain1 = ChainTemplates::analyze_and_commit(input.clone());
        assert!(chain1.last_result().is_none());

        let chain2 = ChainTemplates::list_and_analyze(input.clone());
        assert!(chain2.last_result().is_none());

        let chain3 = ChainTemplates::full_workflow(input);
        assert!(chain3.last_result().is_none());
    }
}
