use crate::hal_prelude::*;
use crate::window::Window;
use gfx_hal::{
    window::SurfaceCapabilities,
    Instance
};

use log::{info};

// To replace with generics when stabilised...
/*
pub type AdapterType = gfx_hal::Adapter<gfx_backend::Backend>;
pub type CommandPoolType = gfx_hal::CommandPool<gfx_backend::Backend, gfx_hal::queue::capability::Graphics>;
pub type DeviceType = gfx_backend::Device;
pub type PhysicalDeviceType = gfx_backend::PhysicalDevice;
pub type QueueType = gfx_hal::queue::family::QueueGroup<gfx_backend::Backend, gfx_hal::queue::capability::Graphics>;
pub type SurfaceCapabilities = ;
pub type ImageViewType = <gfx_backend::Backend as gfx_hal::Backend>::ImageView;
pub type ImageType = <gfx_backend::Backend as gfx_hal::Backend>::Image;
pub type PipelineType = <gfx_backend::Backend as gfx_hal::Backend>::GraphicsPipeline;
pub type SwapchainType = <gfx_backend::Backend as gfx_hal::Backend>::Swapchain;
pub type BackbufferType = gfx_hal::Backbuffer<gfx_backend::Backend>;
pub type FramebufferType = <gfx_backend::Backend as gfx_hal::Backend>::Framebuffer;
pub type SurfaceType = gfx_hal::Surface<gfx_backend::Backend>;
pub type RenderPassType = <gfx_backend::Backend as gfx_hal::Backend>::RenderPass;
*/

pub struct InstanceWrapper {
    instance: gfx_backend::Instance,
}

impl InstanceWrapper {
    pub fn new() -> Self {
        InstanceWrapper {
            instance: gfx_backend::Instance::create("jadis", 1)
        }
    }

    pub fn create_surface(&self, window: &Window) -> <gfx_backend::Backend as gfx_hal::Backend>::Surface {
        self.instance.create_surface(&window.window)
    }

    pub fn create_context(&self, window: &Window) -> Context<gfx_backend::Backend> {
        Context::new(
            self.instance.create_surface(&window.window),
            self.instance.enumerate_adapters())
    }
}

/// Get preferred adapter according to some ordering criterion.
pub fn get_preferred_adapter<B, O, F>(adapters: &[gfx_hal::Adapter<B>], criterion: F) -> usize
    where 
    B: gfx_hal::Backend,
    O: Ord,
    F: FnMut(&(usize, &gfx_hal::Adapter<B>)) -> O {
    adapters.into_iter()
            .enumerate().min_by_key(criterion).unwrap().0
}

pub struct Context<B: gfx_hal::Backend> {
    adapter: usize,
    available_adapters: Vec<gfx_hal::Adapter<B>>,
    pub device: B::Device,
    pub queue_group: gfx_hal::queue::family::QueueGroup<B, gfx_hal::queue::capability::Graphics>,
    pub surface_colour_format: Format,
    pub surface_caps: SurfaceCapabilities,
    pub surface: <B as gfx_hal::Backend>::Surface,
}

impl<B: gfx_hal::Backend> Context<B> {
    pub fn new(surface: <B as gfx_hal::Backend>::Surface, mut available_adapters: Vec<gfx_hal::Adapter<B>>) -> Self {        
        for adapter in &available_adapters {
            info!("Found adapter: {} ({:?})", adapter.info.name, adapter.info.device_type);
        }

        let adapter = get_preferred_adapter(&available_adapters, |(_index, adapter)| {
            use gfx_hal::adapter::DeviceType;
            match adapter.info.device_type {
                DeviceType::IntegratedGpu => 0,
                DeviceType::DiscreteGpu => 1,
                DeviceType::VirtualGpu => 2,
                DeviceType::Cpu => 3,
                DeviceType::Other => 4,
            }
        });
        let (device, physical_device, queue_group) = {
            let actual_adapter = &mut available_adapters[adapter];
            info!("==> Using adapter: {} ({:?})", actual_adapter.info.name, actual_adapter.info.device_type);
            let num_queues = 1;
            let (device, queue_group) = actual_adapter
                .open_with::<_, Graphics>(num_queues, |family| surface.supports_queue_family(family))
                .unwrap();
            let physical_device = &actual_adapter.physical_device;

            (device, physical_device, queue_group)
        };

        let (surface_caps, formats, _) = surface.compatibility(physical_device);
        let surface_colour_format = Context::<B>::pick_surface_colour_format(formats);

        Context {
            adapter,
            available_adapters,
            device,
            queue_group,
            surface_colour_format,
            surface_caps,
            surface,
        }
    }

    pub fn get_compatibility(&self) -> (SurfaceCapabilities, Option<Vec<Format>>, Vec<gfx_hal::PresentMode>) {
        let actual_adapter = &self.available_adapters[self.adapter];
        let physical_device = &actual_adapter.physical_device;
        self.surface.compatibility(physical_device)
    }

    pub fn create_command_pool(&self, max_buffers: usize) -> gfx_hal::CommandPool<B, gfx_hal::queue::capability::Graphics> {
        self.device.create_command_pool_typed(&self.queue_group, CommandPoolCreateFlags::empty(), max_buffers).unwrap()
    }

    pub fn get_swapchain_config(&self) -> SwapchainConfig {
        SwapchainConfig::from_caps(&self.surface_caps, self.surface_colour_format)
    }

    pub fn create_swapchain(&mut self, config: SwapchainConfig, old_swapchain: Option<B::Swapchain>) -> (B::Swapchain, gfx_hal::Backbuffer<B>) {
        self.device.create_swapchain(&mut self.surface, config, old_swapchain).expect("Failed to create swapchain!")
    }

    pub fn map_to_image_views(
        &self,
        images: &[B::Image],
        view_kind: ViewKind,
        swizzle: Swizzle,
        range: SubresourceRange) -> Result<Vec<B::ImageView>, ViewError> {
        images.iter()
                .map(|image| self.create_image_view(image, view_kind, swizzle, range.clone()))
                .collect()
    }

    pub fn create_image_view(
        &self,
        image: &B::Image,
        view_kind: ViewKind,
        swizzle: Swizzle,
        range: SubresourceRange) -> Result<B::ImageView, ViewError> {
        self.device.create_image_view(image,
                                      view_kind,
                                      self.surface_colour_format,
                                      swizzle,
                                      range)
    }

    pub fn image_views_to_fbos(
            &self,
            image_views: &[B::ImageView],
            render_pass: &B::RenderPass,
            extent: Extent) -> Result<Vec<B::Framebuffer>, gfx_hal::device::OutOfMemory> {
        image_views
            .iter()
            .map(|image_view| {
                self.device
                    .create_framebuffer(&render_pass, vec![image_view], extent)
            }).collect()
    }

    /// We pick a colour format from the list of supported formats. If there 
    /// is no list, we default to 'Rgba8Srgb'.
    fn pick_surface_colour_format(formats: Option<Vec<Format>>) -> Format {
        match formats {
                Some(choices) => choices.into_iter()
                                        .find(|format| format.base_format().1 == ChannelType::Srgb)
                                        .unwrap(),
                None => Format::Rgba8Srgb,
            }
    }
}