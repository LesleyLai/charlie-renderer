use ash::extensions::khr;
use ash::extensions::khr::{Surface, Swapchain};
use ash::vk::{
    CommandBufferUsageFlags, Image, ImageView, Offset2D, PhysicalDevice, SurfaceKHR, SwapchainKHR,
};
use ash::{vk, Device, Entry, Instance};
use std::ffi::CStr;
use winit::window::Window;

use vk_shader_macros::include_glsl;

use crate::dyn_result::DynResult;

const TRIANGLE_VERT: &[u32] = include_glsl!("shaders/triangle.vert");
const TRIANGLE_FRAG: &[u32] = include_glsl!("shaders/triangle.frag");

unsafe extern "system" fn vulkan_debug_utils_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let message = std::ffi::CStr::from_ptr((*p_callback_data).p_message);
    let severity = format!("{:?}", message_severity).to_lowercase();
    let ty = format!("{:?}", message_type).to_lowercase();
    println!("[Debug][{}][{}] {:?}", severity, ty, message);
    vk::FALSE
}

fn create_instance(entry: &Entry, window: &Window) -> DynResult<Instance> {
    let app_info = vk::ApplicationInfo {
        api_version: vk::make_api_version(0, 1, 2, 0),
        ..Default::default()
    };

    let layer_names: Vec<std::ffi::CString> =
        vec![std::ffi::CString::new("VK_LAYER_KHRONOS_validation").unwrap()];
    let layer_name_pointers: Vec<*const i8> = layer_names
        .iter()
        .map(|layer_name| layer_name.as_ptr())
        .collect();

    let extensions = {
        let mut extensions = ash_window::enumerate_required_extensions(&window)?;
        extensions.push(ash::extensions::ext::DebugUtils::name());
        extensions
    }
    .iter()
    .map(|cstring| cstring.as_ptr())
    .collect::<Vec<_>>();

    let mut debugcreateinfo = vk::DebugUtilsMessengerCreateInfoEXT::builder()
        .message_severity(
            vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
        )
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
        )
        .pfn_user_callback(Some(vulkan_debug_utils_callback));

    let instance_create_info = vk::InstanceCreateInfo::builder()
        .push_next(&mut debugcreateinfo)
        .application_info(&app_info)
        .enabled_layer_names(&layer_name_pointers)
        .enabled_extension_names(&extensions);

    let instance = unsafe { entry.create_instance(&instance_create_info, None)? };
    Ok(instance)
}

fn find_physical_device(instance: &Instance) -> DynResult<vk::PhysicalDevice> {
    let physical_devices = unsafe { instance.enumerate_physical_devices()? };
    let mut pd_properties_pairs = physical_devices
        .iter()
        .map(|pd| (*pd, unsafe { instance.get_physical_device_properties(*pd) }));
    Ok(pd_properties_pairs
        .clone()
        .find(|(_, prop)| prop.device_type == vk::PhysicalDeviceType::DISCRETE_GPU) // Prefer Discrete
        .or_else(|| pd_properties_pairs.next())
        .expect("Can't find a physical device")
        .0)
}

struct QueueFamilyIndices {
    graphics: u32,
    transfer: u32,
}

fn find_queue_family_indices(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    surface: vk::SurfaceKHR,
    surface_fn: &khr::Surface,
) -> DynResult<QueueFamilyIndices> {
    let queue_family_properties =
        unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
    {
        let mut graphics_qf_index_opt = None;
        let mut transfer_qf_index_opt = None;
        for (index, qfam) in queue_family_properties.iter().enumerate() {
            if qfam.queue_count > 0 {
                if qfam.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                    && unsafe {
                        surface_fn.get_physical_device_surface_support(
                            physical_device,
                            index as u32,
                            surface,
                        )?
                    }
                {
                    graphics_qf_index_opt = Some(index as u32);
                }
                if qfam.queue_flags.contains(vk::QueueFlags::TRANSFER) {
                    if transfer_qf_index_opt.is_none()
                        || !qfam.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                    {
                        transfer_qf_index_opt = Some(index as u32);
                    }
                }
            }
        }
        Ok(QueueFamilyIndices {
            graphics: graphics_qf_index_opt.unwrap(),
            transfer: transfer_qf_index_opt.unwrap(),
        })
    }
}

fn create_device(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    queue_family_indices: &QueueFamilyIndices,
) -> DynResult<Device> {
    let priorities = [1.0f32];
    let queue_infos = [
        vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(queue_family_indices.graphics)
            .queue_priorities(&priorities)
            .build(),
        vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(queue_family_indices.transfer)
            .queue_priorities(&priorities)
            .build(),
    ];

    let extensions: Vec<*const i8> = [
        ash::extensions::khr::Swapchain::name(),
        ash::extensions::khr::DynamicRendering::name(),
    ]
    .iter()
    .map(|name| name.as_ptr())
    .collect::<Vec<_>>();
    let device_create_info = vk::DeviceCreateInfo::builder()
        .queue_create_infos(&queue_infos)
        .enabled_extension_names(&extensions);
    Ok(unsafe { instance.create_device(physical_device, &device_create_info, None) }?)
}

fn create_swapchain(
    instance: &Instance,
    surface: SurfaceKHR,
    surface_fn: &Surface,
    physical_device: PhysicalDevice,
    queue_family_indices: &QueueFamilyIndices,
    device: &Device,
) -> DynResult<(Swapchain, SwapchainKHR, Vec<Image>, Vec<ImageView>)> {
    let surface_capabilities =
        unsafe { surface_fn.get_physical_device_surface_capabilities(physical_device, surface)? };
    let surface_formats =
        unsafe { surface_fn.get_physical_device_surface_formats(physical_device, surface)? };

    let swapcahin_queue_family_indices = [queue_family_indices.graphics];
    let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
        .surface(surface)
        .min_image_count(
            3.max(surface_capabilities.min_image_count)
                .min(surface_capabilities.max_image_count),
        )
        .image_format(surface_formats.first().unwrap().format)
        .image_color_space(surface_formats.first().unwrap().color_space)
        .image_extent(surface_capabilities.current_extent)
        .image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        .queue_family_indices(&swapcahin_queue_family_indices)
        .pre_transform(surface_capabilities.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(vk::PresentModeKHR::FIFO);
    let swapchain_loader = khr::Swapchain::new(&instance, &device);
    let swapchain = unsafe { swapchain_loader.create_swapchain(&swapchain_create_info, None)? };
    let swapchain_images = unsafe { swapchain_loader.get_swapchain_images(swapchain)? };
    let swapchain_image_views = swapchain_images
        .iter()
        .map(|image| {
            let subresource_range = vk::ImageSubresourceRange::builder()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1);
            let imageview_create_info = vk::ImageViewCreateInfo::builder()
                .image(*image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(vk::Format::B8G8R8A8_UNORM)
                .subresource_range(*subresource_range);
            unsafe { device.create_image_view(&imageview_create_info, None) }.unwrap()
        })
        .collect::<Vec<_>>();
    Ok((
        swapchain_loader,
        swapchain,
        swapchain_images,
        swapchain_image_views,
    ))
}

pub struct Renderer {
    entry: Entry,
    instance: Instance,
    surface: vk::SurfaceKHR,
    surface_fn: khr::Surface,
    physical_device: vk::PhysicalDevice,
    device: Device,
    dynamic_rendering_loader: khr::DynamicRendering,
    queue_family_indices: QueueFamilyIndices,
    graphics_queue: vk::Queue,
    transfer_queue: vk::Queue,
    swapchain_loader: khr::Swapchain,
    swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,

    graphics_command_pool: vk::CommandPool,
    main_graphics_command_buffer: vk::CommandBuffer,

    present_semaphore: vk::Semaphore,
    render_semaphore: vk::Semaphore,
    render_fence: vk::Fence,

    frame_number: u64,
}

impl Renderer {
    pub fn new(window: &winit::window::Window) -> DynResult<Renderer> {
        let entry = Entry::linked();

        let instance = create_instance(&entry, &window)?;
        let surface = unsafe { ash_window::create_surface(&entry, &instance, &window, None)? };
        let surface_fn = ash::extensions::khr::Surface::new(&entry, &instance);
        let physical_device = find_physical_device(&instance)?;
        let queue_family_indices =
            find_queue_family_indices(&instance, physical_device, surface, &surface_fn)?;
        let device = create_device(&instance, physical_device, &queue_family_indices)?;

        let dynamic_rendering_loader = khr::DynamicRendering::new(&instance, &device);

        let graphics_queue = unsafe { device.get_device_queue(queue_family_indices.graphics, 0) };
        let transfer_queue = unsafe { device.get_device_queue(queue_family_indices.transfer, 0) };

        let (swapchain_loader, swapchain, swapchain_images, swapchain_image_views) =
            create_swapchain(
                &instance,
                surface,
                &surface_fn,
                physical_device,
                &queue_family_indices,
                &device,
            )?;

        let command_pool_create_info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(queue_family_indices.graphics)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let graphics_command_pool =
            unsafe { device.create_command_pool(&command_pool_create_info, None)? };

        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(graphics_command_pool)
            .command_buffer_count(1)
            .level(vk::CommandBufferLevel::PRIMARY);
        let main_graphics_command_buffer =
            unsafe { device.allocate_command_buffers(&command_buffer_allocate_info) }?[0];

        let semaphore_create_info = vk::SemaphoreCreateInfo::builder();
        let present_semaphore = unsafe { device.create_semaphore(&semaphore_create_info, None) }?;
        let render_semaphore = unsafe { device.create_semaphore(&semaphore_create_info, None) }?;

        let fence_create_info =
            vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);
        let render_fence = unsafe { device.create_fence(&fence_create_info, None) }?;

        let vert_shader_create_info = vk::ShaderModuleCreateInfo::builder().code(TRIANGLE_VERT);
        let triangle_vert_shader =
            unsafe { device.create_shader_module(&vert_shader_create_info, None) }?;

        let frag_shader_create_info = vk::ShaderModuleCreateInfo::builder().code(TRIANGLE_FRAG);
        let triangle_frag_shader =
            unsafe { device.create_shader_module(&frag_shader_create_info, None) }?;

        unsafe {
            device.destroy_shader_module(triangle_vert_shader, None);
            device.destroy_shader_module(triangle_frag_shader, None);
        }

        Ok(Renderer {
            entry,
            instance,
            surface,
            surface_fn,
            physical_device,
            device,
            dynamic_rendering_loader,
            queue_family_indices,
            graphics_queue,
            transfer_queue,
            swapchain_loader,
            swapchain,
            swapchain_images,
            swapchain_image_views,
            graphics_command_pool,
            main_graphics_command_buffer,
            present_semaphore,
            render_semaphore,
            render_fence,
            frame_number: 0u64,
        })
    }

    pub fn render(&mut self) -> DynResult<()> {
        const ONE_SECOND_IN_NANO_SECONDS: u64 = 1_000_000_000;
        let render_fence_array = [self.render_fence];
        unsafe {
            self.device
                .wait_for_fences(&render_fence_array, true, ONE_SECOND_IN_NANO_SECONDS)?;
            self.device.reset_fences(&render_fence_array)?;
        }

        let (swapchain_image_index, _) = unsafe {
            self.swapchain_loader.acquire_next_image(
                self.swapchain,
                ONE_SECOND_IN_NANO_SECONDS,
                self.present_semaphore,
                vk::Fence::null(),
            )?
        };

        unsafe {
            self.device.reset_command_buffer(
                self.main_graphics_command_buffer,
                vk::CommandBufferResetFlags::empty(),
            )?
        }

        let command_buffer_begin_info =
            vk::CommandBufferBeginInfo::builder().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            self.device.begin_command_buffer(
                self.main_graphics_command_buffer,
                &command_buffer_begin_info,
            )
        }?;

        let color_subresource_range = vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        };
        let image_memory_barrier = vk::ImageMemoryBarrier::builder()
            .image(self.swapchain_images[swapchain_image_index as usize])
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .subresource_range(color_subresource_range)
            .build();

        unsafe {
            self.device.cmd_pipeline_barrier(
                self.main_graphics_command_buffer,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[image_memory_barrier],
            )
        }

        let flash = f32::abs(f32::sin(self.frame_number as f32 / 50f32));
        let clear_values = vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, flash, 1.0],
            },
        };

        let color_attachments = [vk::RenderingAttachmentInfoKHR::builder()
            .clear_value(clear_values)
            .image_view(self.swapchain_image_views[swapchain_image_index as usize])
            .image_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL_KHR)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .build()];
        let render_info = vk::RenderingInfoKHR::builder()
            // flags
            .render_area(vk::Rect2D {
                extent: vk::Extent2D {
                    width: 800,
                    height: 600,
                }, // TODO: window extend
                offset: Offset2D { x: 0, y: 0 },
            })
            .layer_count(1)
            .color_attachments(&color_attachments);

        unsafe {
            self.dynamic_rendering_loader
                .cmd_begin_rendering(self.main_graphics_command_buffer, &render_info);
        }

        unsafe {
            self.dynamic_rendering_loader
                .cmd_end_rendering(self.main_graphics_command_buffer);
        }

        let image_memory_barrier = vk::ImageMemoryBarrier::builder()
            .image(self.swapchain_images[swapchain_image_index as usize])
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_access_mask(vk::AccessFlags::empty())
            .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .subresource_range(color_subresource_range)
            .build();

        unsafe {
            self.device.cmd_pipeline_barrier(
                self.main_graphics_command_buffer,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[image_memory_barrier],
            )
        }

        unsafe {
            self.device
                .end_command_buffer(self.main_graphics_command_buffer)?;
        }

        // Submit
        let sumbit_info = vk::SubmitInfo::builder()
            .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
            .wait_semaphores(&[self.present_semaphore])
            .signal_semaphores(&[self.render_semaphore])
            .command_buffers(&[self.main_graphics_command_buffer])
            .build();
        unsafe {
            self.device
                .queue_submit(self.graphics_queue, &[sumbit_info], self.render_fence)
        }?;

        // Present
        let present_swapchains = [self.swapchain];
        let present_wait_semaphore = [self.render_semaphore];
        let present_swapchain_image_indices = [swapchain_image_index];

        let present_info = vk::PresentInfoKHR::builder()
            .swapchains(&present_swapchains)
            .wait_semaphores(&present_wait_semaphore)
            .image_indices(&present_swapchain_image_indices);
        unsafe {
            self.swapchain_loader
                .queue_present(self.graphics_queue, &present_info)
        }?;

        // begin render pass
        self.frame_number += 1;
        Ok(())
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();

            self.device.destroy_semaphore(self.render_semaphore, None);
            self.device.destroy_semaphore(self.present_semaphore, None);
            self.device.destroy_fence(self.render_fence, None);

            self.device
                .destroy_command_pool(self.graphics_command_pool, None);

            self.swapchain_image_views
                .iter()
                .for_each(|image_view| self.device.destroy_image_view(*image_view, None));
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);

            self.device.destroy_device(None);
            self.surface_fn.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}
