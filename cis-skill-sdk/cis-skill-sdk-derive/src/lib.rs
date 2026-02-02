//! CIS Skill SDK Derive Macros

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemImpl};

/// `#[skill]` 宏
///
/// 自动为 Skill 实现注册和导出功能
///
/// # 示例
///
/// ```rust
/// use cis_skill_sdk::{Skill, SkillContext, Event, Result};
///
/// pub struct MySkill;
///
/// #[cis_skill_sdk::skill]
/// impl Skill for MySkill {
///     fn name(&self) -> &str { "my-skill" }
///     fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
///         Ok(())
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn skill(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemImpl);
    
    // 获取类型名称
    let self_ty = &input.self_ty;
    
    // 生成额外代码
    let expanded = quote! {
        #input
        
        // Native 模式：自动注册
        #[cfg(feature = "native")]
        ::inventory::submit! {
            ::cis_skill_sdk::skill::SkillRegistration::native::<#self_ty>(
                <#self_ty as ::cis_skill_sdk::Skill>::name(&#self_ty),
                <#self_ty as ::cis_skill_sdk::Skill>::version(&#self_ty),
            )
        }
    };
    
    TokenStream::from(expanded)
}

/// `#[derive(Skill)]` 宏（简化版）
///
/// 为 struct 自动生成 Skill 实现
#[proc_macro_derive(Skill, attributes(skill))]
pub fn derive_skill(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_skill(&ast)
}

fn impl_skill(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    
    // 查找 skill 属性
    let mut skill_name = None;
    let mut skill_version = None;
    let mut skill_description = None;
    
    for attr in &ast.attrs {
        if attr.path().is_ident("skill") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    skill_name = Some(value.value());
                    Ok(())
                } else if meta.path.is_ident("version") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    skill_version = Some(value.value());
                    Ok(())
                } else if meta.path.is_ident("description") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    skill_description = Some(value.value());
                    Ok(())
                } else {
                    Err(meta.error("unsupported skill attribute"))
                }
            }).ok();
        }
    }
    
    let name_str = skill_name.unwrap_or_else(|| name.to_string().to_lowercase());
    let version_str = skill_version.unwrap_or_else(|| "0.1.0".to_string());
    let desc_str = skill_description.unwrap_or_default();
    
    let gen = quote! {
        impl ::cis_skill_sdk::Skill for #name {
            fn name(&self) -> &str {
                #name_str
            }
            
            fn version(&self) -> &str {
                #version_str
            }
            
            fn description(&self) -> &str {
                #desc_str
            }
            
            fn handle_event(&self, ctx: &dyn ::cis_skill_sdk::SkillContext, event: ::cis_skill_sdk::Event) -> ::cis_skill_sdk::Result<()> {
                // 默认空实现，用户需手动实现
                Ok(())
            }
        }
    };
    
    gen.into()
}
