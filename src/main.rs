use ash::{vk, Entry, Instance};
use ash::vk::{PhysicalDevice, PhysicalDeviceFeatures, PhysicalDeviceProperties};

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
    let pd_properties_pairs =
        physical_devices
            .iter()
            .map(|pd| (*pd, unsafe { instance.get_physical_device_properties(*pd) }));
    Ok(pd_properties_pairs.clone()
        .find(|(pd, prop)| prop.device_type == vk::PhysicalDeviceType::DISCRETE_GPU) // Prefer Discrete
        .or_else(|| pd_properties_pairs.clone().next())
        .expect("Can't find a physical device"))
}

fn main() -> DynResult<()> {
    let instance = create_instance()?;
    let (physical_device, physical_device_properties) = find_physical_device(&instance)?;


    unsafe { instance.destroy_instance(None) };
    Ok(())
}
