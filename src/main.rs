use ash::{vk, Entry, Instance, Device};
use ash::vk::{PhysicalDevice, PhysicalDeviceFeatures, PhysicalDeviceProperties, Queue};

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

fn create_instance() -> DynResult<Instance> {
    let entry = Entry::linked();
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
    let extension_name_pointers: Vec<*const i8> =
        vec![ash::extensions::ext::DebugUtils::name().as_ptr()];

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
        .enabled_extension_names(&extension_name_pointers);

    let instance = unsafe { entry.create_instance(&instance_create_info, None)? };
    Ok(instance)
}

fn find_physical_device(instance: &Instance) -> DynResult<(PhysicalDevice, PhysicalDeviceProperties)> {
    let physical_devices = unsafe { instance.enumerate_physical_devices()? };
    let mut pd_properties_pairs =
        physical_devices
            .iter()
            .map(|pd| (*pd, unsafe { instance.get_physical_device_properties(*pd) }));
    Ok(pd_properties_pairs.clone()
        .find(|(pd, prop)| prop.device_type == vk::PhysicalDeviceType::DISCRETE_GPU) // Prefer Discrete
        .or_else(|| pd_properties_pairs.next())
        .expect("Can't find a physical device"))
}

struct QueueFamilyIndices {
    graphics: u32,
    transfer: u32,
}

fn find_queue_family_indices(instance: &Instance, physical_device: PhysicalDevice)
                             -> QueueFamilyIndices {
    let queue_family_properties =
        unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
    {
        let mut graphics_qf_index_opt = None;
        let mut transfer_qf_index_opt = None;
        for (index, qfam) in queue_family_properties.iter().enumerate() {
            if qfam.queue_count > 0 {
                if qfam.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
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
        QueueFamilyIndices {
            graphics: graphics_qf_index_opt.unwrap(),
            transfer: transfer_qf_index_opt.unwrap(),
        }
    }
}


fn main() -> DynResult<()> {
    let instance = create_instance()?;
    let (physical_device, physical_device_properties) = find_physical_device(&instance)?;
    let queue_family_indices =
        find_queue_family_indices(&instance, physical_device);
    let (logical_device, graphics_queue, transfer_queue)
        = create_logical_device(&instance, physical_device, queue_family_indices)?;

    let eventloop = winit::event_loop::EventLoop::new();
    let window = winit::window::Window::new(&eventloop)?;

    unsafe {
        logical_device.destroy_device(None);
        instance.destroy_instance(None);
    };
    Ok(())
}

fn create_logical_device(instance: &Instance, physical_device: PhysicalDevice, queue_family_indices: QueueFamilyIndices)
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
    let device_create_info = vk::DeviceCreateInfo::builder()
        .queue_create_infos(&queue_infos);
    let logical_device =
        unsafe { instance.create_device(physical_device, &device_create_info, None)? };
    let graphics_queue = unsafe { logical_device.get_device_queue(queue_family_indices.graphics, 0) };
    let transfer_queue = unsafe { logical_device.get_device_queue(queue_family_indices.transfer, 0) };
    Ok((logical_device, graphics_queue, transfer_queue))
}

