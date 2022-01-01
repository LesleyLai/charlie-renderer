use ash::{vk, Entry, Instance, Device};
use ash::extensions::khr::Surface;
use ash::vk::{PhysicalDevice, Queue, SurfaceKHR};

type DynResult<T> = Result<T, Box<dyn std::error::Error>>;

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

fn create_instance(entry: &Entry, window: &winit::window::Window) -> DynResult<Instance> {
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
    }.iter().map(|cstring| cstring.as_ptr()).collect::<Vec<_>>();


    let mut debugcreateinfo =
        vk::DebugUtilsMessengerCreateInfoEXT::builder()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR)
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION)
            .pfn_user_callback(Some(vulkan_debug_utils_callback));

    let instance_create_info = vk::InstanceCreateInfo::builder()
        .push_next(&mut debugcreateinfo)
        .application_info(&app_info)
        .enabled_layer_names(&layer_name_pointers)
        .enabled_extension_names(&extensions);

    let instance = unsafe { entry.create_instance(&instance_create_info, None)? };
    Ok(instance)
}

fn find_physical_device(instance: &Instance) -> DynResult<PhysicalDevice> {
    let physical_devices = unsafe { instance.enumerate_physical_devices()? };
    let mut pd_properties_pairs =
        physical_devices
            .iter()
            .map(|pd| (*pd, unsafe { instance.get_physical_device_properties(*pd) }));
    Ok(pd_properties_pairs.clone()
        .find(|(_, prop)| prop.device_type == vk::PhysicalDeviceType::DISCRETE_GPU) // Prefer Discrete
        .or_else(|| pd_properties_pairs.next())
        .expect("Can't find a physical device")
        .0
    )
}

struct QueueFamilyIndices {
    graphics: u32,
    transfer: u32,
}

fn find_queue_family_indices(instance: &Instance,
                             physical_device: PhysicalDevice,
                             surface: SurfaceKHR,
                             surface_fn: &Surface)
                             -> DynResult<QueueFamilyIndices> {
    let queue_family_properties =
        unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
    {
        let mut graphics_qf_index_opt = None;
        let mut transfer_qf_index_opt = None;
        for (index, qfam) in queue_family_properties.iter().enumerate() {
            if qfam.queue_count > 0 {
                if qfam.queue_flags.contains(vk::QueueFlags::GRAPHICS) && unsafe {
                    surface_fn
                        .get_physical_device_surface_support(physical_device, index as u32, surface)?
                } {
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

fn create_logical_device(instance: &Instance,
                         physical_device: PhysicalDevice,
                         queue_family_indices: &QueueFamilyIndices)
                         -> DynResult<(Device, Queue, Queue)> {
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
    let extensions: Vec<*const i8> = vec![ash::extensions::khr::Swapchain::name().as_ptr()];
    let device_create_info = vk::DeviceCreateInfo::builder()
        .queue_create_infos(&queue_infos)
        .enabled_extension_names(&extensions);
    let logical_device =
        unsafe { instance.create_device(physical_device, &device_create_info, None)? };
    let graphics_queue = unsafe { logical_device.get_device_queue(queue_family_indices.graphics, 0) };
    let transfer_queue = unsafe { logical_device.get_device_queue(queue_family_indices.transfer, 0) };
    Ok((logical_device, graphics_queue, transfer_queue))
}

fn main() -> DynResult<()> {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::Window::new(&event_loop)?;

    let entry = Entry::linked();

    let instance = create_instance(&entry, &window)?;
    let surface = unsafe { ash_window::create_surface(&entry, &instance, &window, None)? };
    let surface_fn = ash::extensions::khr::Surface::new(&entry, &instance);
    let physical_device = find_physical_device(&instance)?;
    let queue_family_indices =
        find_queue_family_indices(&instance, physical_device, surface, &surface_fn)?;
    let (logical_device, _graphics_queue, _transfer_queue)
        = create_logical_device(&instance, physical_device, &queue_family_indices)?;

    let surface_capabilities = unsafe {
        surface_fn.get_physical_device_surface_capabilities(physical_device, surface)?
    };
    let surface_formats = unsafe {
        surface_fn.get_physical_device_surface_formats(physical_device, surface)?
    };

    let queuefamilies = [queue_family_indices.graphics];
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
        .queue_family_indices(&queuefamilies)
        .pre_transform(surface_capabilities.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(vk::PresentModeKHR::FIFO);
    let swapchain_loader = ash::extensions::khr::Swapchain::new(&instance, &logical_device);
    let swapchain = unsafe { swapchain_loader.create_swapchain(&swapchain_create_info, None)? };
    let swapchain_images = unsafe { swapchain_loader.get_swapchain_images(swapchain)? };
    let swapchain_image_views =
        swapchain_images.iter().map(
            |image| {
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
                unsafe { logical_device.create_image_view(&imageview_create_info, None) }.unwrap()
            }
        ).collect::<Vec<_>>();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = winit::event_loop::ControlFlow::Wait;

        match event {
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => {
                unsafe {
                    swapchain_image_views.iter().for_each(
                        |image_view| logical_device.destroy_image_view(*image_view, None)
                    );
                    swapchain_loader.destroy_swapchain(swapchain, None);

                    logical_device.destroy_device(None);
                    surface_fn.destroy_surface(surface, None);
                    instance.destroy_instance(None);
                };
                *control_flow = winit::event_loop::ControlFlow::Exit
            }
            _ => (),
        }
    });
}
