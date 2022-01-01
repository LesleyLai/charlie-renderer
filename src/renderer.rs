use ash::{vk, Entry, Instance, Device};
use ash::extensions::khr;
use winit::window::Window;

use crate::dyn_result::DynResult;

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

fn find_physical_device(instance: &Instance) -> DynResult<vk::PhysicalDevice> {
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
                             physical_device: vk::PhysicalDevice,
                             surface: vk::SurfaceKHR,
                             surface_fn: &khr::Surface)
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

fn create_device(instance: &Instance,
                 physical_device: vk::PhysicalDevice,
                 queue_family_indices: &QueueFamilyIndices) -> DynResult<Device> {
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
    Ok(unsafe { instance.create_device(physical_device, &device_create_info, None) }?)
}

pub struct Renderer {
    entry: Entry,
    instance: Instance,
    surface: vk::SurfaceKHR,
    surface_fn: khr::Surface,
    physical_device: vk::PhysicalDevice,
    device: Device,
    queue_family_indices: QueueFamilyIndices,
    graphics_queue: vk::Queue,
    transfer_queue: vk::Queue,
    swapchain_loader: khr::Swapchain,
    swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
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
        let graphics_queue = unsafe { device.get_device_queue(queue_family_indices.graphics, 0) };
        let transfer_queue = unsafe { device.get_device_queue(queue_family_indices.transfer, 0) };

        let surface_capabilities = unsafe {
            surface_fn.get_physical_device_surface_capabilities(physical_device, surface)?
        };
        let surface_formats = unsafe {
            surface_fn.get_physical_device_surface_formats(physical_device, surface)?
        };

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
                    unsafe { device.create_image_view(&imageview_create_info, None) }.unwrap()
                }
            ).collect::<Vec<_>>();

        Ok(Renderer {
            entry,
            instance,
            surface,
            surface_fn,
            physical_device,
            device,
            queue_family_indices,
            graphics_queue,
            transfer_queue,
            swapchain_loader,
            swapchain,
            swapchain_images,
            swapchain_image_views,
        })
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.swapchain_image_views.iter().for_each(
                |image_view| self.device.destroy_image_view(*image_view, None)
            );
            self.swapchain_loader.destroy_swapchain(self.swapchain, None);

            self.device.destroy_device(None);
            self.surface_fn.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}